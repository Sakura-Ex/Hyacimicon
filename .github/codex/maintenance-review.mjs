import { spawnSync } from 'node:child_process';
import { appendFileSync, readFileSync } from 'node:fs';

const { OPENAI_API_KEY: apiKey, ...gitEnv } = process.env;
const { PR_BASE: base, PR_MERGE_BASE: mergeBase, PR_HEAD: head, GITHUB_OUTPUT: outputFile } = gitEnv;
const maxQueries = 8;
const deadline = AbortSignal.timeout(12 * 60_000);

function git(args, allowedStatuses = [0]) {
  const result = spawnSync('git', ['--literal-pathspecs', '-c', 'core.quotePath=false', ...args], {
    env: gitEnv,
    encoding: 'utf8',
    timeout: 10_000,
    maxBuffer: 4 * 1024 * 1024,
  });
  if (result.error || !allowedStatuses.includes(result.status)) {
    throw new Error('Git could not read this object or the requested output exceeded its limits.');
  }
  return result.stdout;
}

function filePath(value) {
  if (typeof value !== 'string' || !value || value.length > 1024 ||
      /[\x00-\x1f\x7f]/.test(value) ||
      value.split('/').some((part) => ['', '.', '..'].includes(part))) {
    throw new Error('Use a literal path relative to the repository root.');
  }
  return value;
}

function page(text, startLine, { maxChars = 12_000, maxLines = 240 } = {}) {
  if (!Number.isSafeInteger(startLine) || startLine < 1) {
    throw new Error('Start line must be a positive integer.');
  }
  const lines = text.split('\n');
  const excerpt = [];
  let size = 0;
  let index = startLine - 1;
  let truncatedLine = false;
  while (index < lines.length && excerpt.length < maxLines) {
    const line = lines[index];
    if (excerpt.length && size + line.length + 1 > maxChars) break;
    if (line.length >= maxChars) {
      truncatedLine = true;
      excerpt.push(`${line.slice(0, maxChars - 25)} [line truncated]`);
    } else {
      excerpt.push(line);
    }
    size += excerpt.at(-1).length + 1;
    index += 1;
  }
  return {
    first_line: startLine, text: excerpt.join('\n'),
    next_line: index < lines.length ? index + 1 : null, truncated_line: truncatedLine,
  };
}

const diffArgs = ['diff', '--no-color', '--no-ext-diff', '--no-textconv', '--submodule=short', '-M'];
const pathParam = { type: 'string', description: 'Literal path relative to the repository root' };
const lineParam = { type: 'integer', description: 'First line to return, starting at 1' };
const tools = [
  {
    name: 'read_diff',
    description: 'Read a page of the PR diff or file list. Use null path for the entire comparison.',
    properties: {
      path: { ...pathParam, type: ['string', 'null'] },
      view: { type: 'string', enum: ['patch', 'files'] },
      start_line: lineParam,
    },
    read: ({ path, view, start_line }, limits) => {
      if (!['patch', 'files'].includes(view)) throw new Error('Use the patch or files view.');
      const paths = path === null ? [] : [filePath(path)];
      const format = view === 'files' ? ['--name-status'] : [];
      return page(git([...diffArgs, ...format, mergeBase, head, '--', ...paths]), start_line, limits);
    },
  },
  {
    name: 'read_file',
    description: 'Read numbered lines of a tracked file from the target or PR snapshot.',
    properties: { revision: { type: 'string', enum: ['base', 'head'] }, path: pathParam, start_line: lineParam },
    read: ({ revision, path, start_line }) => {
      const commit = revision === 'base' ? base : revision === 'head' ? head : null;
      if (!commit) throw new Error('Use the base or head snapshot.');
      const content = git(['cat-file', 'blob', `${commit}:${filePath(path)}`]);
      if (content.includes('\0')) throw new Error('The requested file is binary.');
      return page(content, start_line);
    },
  },
  {
    name: 'search_code',
    description: 'Find a literal string and its line numbers in tracked PR files. A directory limits the search.',
    properties: {
      query: { type: 'string', description: 'Literal search term' },
      path: { ...pathParam, type: ['string', 'null'] },
      start_line: lineParam,
    },
    read: ({ query, path, start_line }) => {
      if (typeof query !== 'string' || !query.trim() || query.length > 200 || /[\x00-\x1f\x7f]/.test(query)) {
        throw new Error('Use one literal search term of at most 200 characters.');
      }
      const paths = path === null ? [] : [filePath(path)];
      return page(git(['grep', '--no-textconv', '--no-recurse-submodules', '-nIF', '-e', query, head, '--', ...paths], [0, 1]), start_line);
    },
  },
];

function readEvidence(name, args, limits) {
  try {
    return tools.find((tool) => tool.name === name).read(JSON.parse(args), limits);
  } catch (error) {
    if (error instanceof TypeError || error instanceof SyntaxError) {
      return { error: 'Invalid evidence request.' };
    }
    return { error: error.message };
  }
}

async function createResponse(payload) {
  const http = await fetch('https://api.openai.com/v1/responses', {
    method: 'POST',
    redirect: 'error',
    headers: { Authorization: `Bearer ${apiKey}`, 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
    signal: AbortSignal.any([deadline, AbortSignal.timeout(120_000)]),
  });
  const response = await http.json().catch(() => null);
  if (!http.ok) {
    const code = response?.error?.code;
    const detail = typeof code === 'string' && /^[a-z][a-z0-9_]{0,79}$/.test(code) ? ` (${code})` : '';
    throw new Error(`Responses API returned HTTP ${http.status}${detail}.`);
  }
  if (response?.status !== 'completed' || !Array.isArray(response.output)) {
    throw new Error('Responses API did not complete the review.');
  }
  return response.output;
}

async function review() {
  if (!apiKey || !outputFile || ![base, mergeBase, head].every((sha) => /^[0-9a-f]{40}$/.test(sha))) {
    throw new Error('Maintenance review is missing its API key, output file, or commit IDs.');
  }
  const prompt = readFileSync(new URL('./maintenance-review.md', import.meta.url), 'utf8');
  const schema = JSON.parse(readFileSync(new URL('./maintenance-review.schema.json', import.meta.url), 'utf8'));
  const references = ['AGENTS.md', 'docs/development/project-layout.md', 'docs/development/development-guide.md'];
  const conventions = references.map((path) => `## ${path}\n\n${git(['cat-file', 'blob', `${base}:${path}`])}`);
  const initialLimits = { maxChars: 40_000, maxLines: 1_200 };
  const files = readEvidence('read_diff', '{"path":null,"view":"files","start_line":1}', initialLimits);
  const patch = readEvidence('read_diff', '{"path":null,"view":"patch","start_line":1}', initialLimits);
  const partialDiff = [files, patch].some((part) => part.error || part.next_line || part.truncated_line);
  const input = [
    { role: 'developer', content: [prompt, ...conventions].join('\n\n') },
    { role: 'user', content: JSON.stringify({ base, merge_base: mergeBase, head, files, patch }) },
  ];
  const functionTools = tools.map(({ name, description, properties }) => ({
    type: 'function', name, description, strict: true,
    parameters: { type: 'object', properties, required: Object.keys(properties), additionalProperties: false },
  }));

  for (let queryCount = 0; queryCount <= maxQueries; queryCount += 1) {
    const finalTurn = queryCount === maxQueries;
    const output = await createResponse({
      model: 'gpt-6.2-sol', reasoning: { effort: 'medium' }, max_output_tokens: 8192,
      input,
      store: false, include: ['reasoning.encrypted_content'],
      tools: functionTools, parallel_tool_calls: false, tool_choice: finalTurn ? 'none' : 'auto',
      text: { format: { type: 'json_schema', name: 'maintenance_review', strict: true, schema } },
    });
    const calls = output.filter((item) => item.type === 'function_call');
    if (!calls.length) {
      const answer = output.filter((item) => item.type === 'message')
        .flatMap((item) => item.content).filter((item) => item.type === 'output_text')
        .map((item) => item.text).join('');
      if (!answer) throw new Error('Responses API did not return a structured review.');
      const result = JSON.parse(answer);
      if (partialDiff) {
        result.coverage = 'partial';
        result.summary = `PR 差异超出本次预读范围，结论覆盖已核对的内容。${result.summary}`;
      }
      return result;
    }
    if (finalTurn || calls.length !== 1) throw new Error('Responses API returned unexpected evidence requests.');
    const [call] = calls;
    input.push(...output, {
      type: 'function_call_output', call_id: call.call_id,
      output: JSON.stringify(readEvidence(call.name, call.arguments)),
    });
  }
}

try {
  const result = await review();
  appendFileSync(outputFile, `result=${JSON.stringify(result)}\n`);
} catch (error) {
  console.error(error instanceof SyntaxError ? 'Invalid review JSON.' : error.message);
  process.exitCode = 1;
}
