/**
 * Deterministic PRNG (FNV-1a 32-bit seed hash + Mulberry32) for reproducible
 * shop rolls, crystal drops, and battle attempt streams.
 */
function hashSeed(seed: string): number {
  let hash = 0x811c9dc5;
  for (let i = 0; i < seed.length; i += 1) {
    hash ^= seed.charCodeAt(i);
    hash = Math.imul(hash, 0x01000193);
  }
  return hash >>> 0;
}

export class SeededRng {
  private state: number;

  public constructor(seed: string) {
    this.state = hashSeed(seed) || 0x9e3779b9;
  }

  public nextFloat(): number {
    this.state = (this.state + 0x6d2b79f5) >>> 0;
    let t = this.state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  }

  public nextInt(maxExclusive: number): number {
    if (maxExclusive <= 0) {
      return 0;
    }
    return Math.floor(this.nextFloat() * maxExclusive);
  }

  public pick<T>(items: readonly T[]): T | undefined {
    if (items.length === 0) {
      return undefined;
    }
    return items[this.nextInt(items.length)];
  }

  public shuffle<T>(items: readonly T[]): T[] {
    const copy = [...items];
    for (let i = copy.length - 1; i > 0; i -= 1) {
      const j = this.nextInt(i + 1);
      const a = copy[i];
      const b = copy[j];
      if (a !== undefined && b !== undefined) {
        copy[i] = b;
        copy[j] = a;
      }
    }
    return copy;
  }

  public fork(label: string): SeededRng {
    const sub = this.nextInt(0x7fffffff);
    return new SeededRng(`${label}:${sub}`);
  }
}
