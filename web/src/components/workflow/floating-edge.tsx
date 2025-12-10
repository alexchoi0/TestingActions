'use client';

import {
  BaseEdge,
  EdgeProps,
  useInternalNode,
  getSmoothStepPath,
  type InternalNode,
  type Node,
} from '@xyflow/react';

function getNodeCenter(node: InternalNode<Node>) {
  const positionAbsolute = node.internals.positionAbsolute;
  const w = node.measured?.width ?? 0;
  const h = node.measured?.height ?? 0;

  return {
    x: positionAbsolute.x + w / 2,
    y: positionAbsolute.y + h / 2,
  };
}

function getNodeIntersection(
  node: InternalNode<Node>,
  targetPoint: { x: number; y: number }
) {
  const positionAbsolute = node.internals.positionAbsolute;
  const w = node.measured?.width ?? 0;
  const h = node.measured?.height ?? 0;

  const x = positionAbsolute.x + w / 2;
  const y = positionAbsolute.y + h / 2;

  const dx = targetPoint.x - x;
  const dy = targetPoint.y - y;

  if (dx === 0 && dy === 0) {
    return { x, y };
  }

  const absDx = Math.abs(dx);
  const absDy = Math.abs(dy);

  if (h === 0) {
    return { x, y };
  }

  const aspectRatio = w / h;

  let intersectX: number;
  let intersectY: number;

  if (absDy * aspectRatio > absDx) {
    const sign = dy > 0 ? 1 : -1;
    intersectY = y + (h / 2) * sign;
    intersectX = x + (dx * (h / 2)) / absDy;
  } else {
    const sign = dx > 0 ? 1 : -1;
    intersectX = x + (w / 2) * sign;
    intersectY = y + (dy * (w / 2)) / absDx;
  }

  return { x: intersectX, y: intersectY };
}

function getEdgeParams(
  source: InternalNode<Node>,
  target: InternalNode<Node>
) {
  const sourceCenter = getNodeCenter(source);
  const targetCenter = getNodeCenter(target);

  const sourceIntersection = getNodeIntersection(source, targetCenter);
  const targetIntersection = getNodeIntersection(target, sourceCenter);

  return {
    sx: sourceIntersection.x,
    sy: sourceIntersection.y,
    tx: targetIntersection.x,
    ty: targetIntersection.y,
  };
}

export default function FloatingEdge({
  id,
  source,
  target,
  markerEnd,
  style,
}: EdgeProps) {
  const sourceNode = useInternalNode(source);
  const targetNode = useInternalNode(target);

  if (!sourceNode || !targetNode) {
    return null;
  }

  const { sx, sy, tx, ty } = getEdgeParams(sourceNode, targetNode);

  const [edgePath] = getSmoothStepPath({
    sourceX: sx,
    sourceY: sy,
    targetX: tx,
    targetY: ty,
    borderRadius: 8,
  });

  return (
    <BaseEdge
      id={id}
      path={edgePath}
      markerEnd={markerEnd}
      style={{
        ...style,
        strokeDasharray: 5,
        animation: 'dashedFlow 0.5s linear infinite',
      }}
    />
  );
}
