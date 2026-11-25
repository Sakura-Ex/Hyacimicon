import type { NodeKind, RouteNodeItem } from '../../types/hsr';

const NODE_GLYPH: Record<NodeKind, string> = {
  reward: '◈',
  battle: '⚔',
  encounter: '✧',
  supply: '❖',
  boss: '♛',
};

const NODE_LABEL: Record<NodeKind, string> = {
  reward: '奖励',
  battle: '战斗',
  encounter: '遭遇',
  supply: '补给',
  boss: '首领',
};

export interface RouteTimelineProps {
  nodes: RouteNodeItem[];
}

export function RouteTimeline({ nodes }: RouteTimelineProps) {
  return (
    <nav className="hsr-route-timeline" aria-label="位面节点路线">
      {nodes.map((node, index) => {
        const prev = nodes[index - 1];
        const showPlaneSep = prev !== undefined && prev.plane !== node.plane;

        const stateClass =
          node.status === 'done'
            ? 'hsr-route-node--done'
            : node.status === 'current'
              ? 'hsr-route-node--current'
              : '';

        const kindClass =
          node.kind === 'boss'
            ? 'hsr-route-node--boss'
            : node.kind === 'supply'
              ? 'hsr-route-node--supply'
              : '';

        return (
          <div key={node.nodeId} style={{ display: 'inline-flex', alignItems: 'center', gap: '3px' }}>
            {showPlaneSep && (
              <span className="hsr-route-plane-sep" aria-hidden="true">
                ◆
              </span>
            )}
            <span
              className={`hsr-route-node ${stateClass} ${kindClass}`.trim()}
              title={`${node.nodeId} · ${NODE_LABEL[node.kind]}`}
            >
              <span aria-hidden="true">{NODE_GLYPH[node.kind]}</span>
              <span className="hsr-route-node-id">{node.nodeId}</span>
            </span>
          </div>
        );
      })}
    </nav>
  );
}
