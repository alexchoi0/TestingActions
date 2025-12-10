'use client';

import { useCallback, useMemo } from 'react';
import {
  ReactFlow,
  Background,
  Controls,
  useNodesState,
  useEdgesState,
  type Node,
  type Edge,
  MarkerType,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import Dagre from '@dagrejs/dagre';

import WorkflowNode from './workflow-node';
import FloatingEdge from './floating-edge';
import type { Workflow } from '@/types/workflow';

const nodeTypes = {
  workflow: WorkflowNode,
};

const edgeTypes = {
  floating: FloatingEdge,
};

interface WorkflowDAGProps {
  workflows: Record<string, Workflow>;
  onWorkflowSelect?: (workflowId: string) => void;
}

const nodeWidth = 220;
const nodeHeight = 100;

function getLayoutedElements(nodes: Node[], edges: Edge[]) {
  const g = new Dagre.graphlib.Graph().setDefaultEdgeLabel(() => ({}));

  g.setGraph({
    rankdir: 'LR',
    nodesep: 80,
    ranksep: 200,
    marginx: 40,
    marginy: 40,
  });

  nodes.forEach((node) => {
    g.setNode(node.id, { width: nodeWidth, height: nodeHeight });
  });

  edges.forEach((edge) => {
    g.setEdge(edge.source, edge.target);
  });

  Dagre.layout(g);

  const layoutedNodes = nodes.map((node) => {
    const nodeWithPosition = g.node(node.id);
    return {
      ...node,
      position: {
        x: nodeWithPosition.x - nodeWidth / 2,
        y: nodeWithPosition.y - nodeHeight / 2,
      },
    };
  });

  return { nodes: layoutedNodes, edges };
}

function buildDAG(workflows: Record<string, Workflow>): { nodes: Node[]; edges: Edge[] } {
  const workflowList = Object.values(workflows);

  const nodes: Node[] = workflowList.map((workflow) => {
    const jobCount = Object.keys(workflow.jobs).length;
    const completedJobs = Object.values(workflow.jobs).filter(
      j => j.status === 'success' || j.status === 'failed' || j.status === 'skipped'
    ).length;

    return {
      id: workflow.name,
      type: 'workflow',
      position: { x: 0, y: 0 },
      data: {
        name: workflow.name,
        status: workflow.status,
        jobCount,
        completedJobs,
      },
    };
  });

  const edges: Edge[] = [];
  workflowList.forEach(workflow => {
    workflow.dependsOn.forEach(dep => {
      edges.push({
        id: `${dep}-${workflow.name}`,
        source: dep,
        target: workflow.name,
        type: 'floating',
        markerEnd: { type: MarkerType.ArrowClosed },
        style: { stroke: '#71717a' },
      });
    });
  });

  return getLayoutedElements(nodes, edges);
}

export default function WorkflowDAG({ workflows, onWorkflowSelect }: WorkflowDAGProps) {
  const { nodes: initialNodes, edges: initialEdges } = useMemo(
    () => buildDAG(workflows),
    [workflows]
  );

  const [nodes, setNodes, onNodesChange] = useNodesState(initialNodes);
  const [edges, setEdges, onEdgesChange] = useEdgesState(initialEdges);

  const onNodeClick = useCallback(
    (_: React.MouseEvent, node: Node) => {
      onWorkflowSelect?.(node.id);
    },
    [onWorkflowSelect]
  );

  return (
    <div className="h-full w-full">
      <ReactFlow
        nodes={nodes}
        edges={edges}
        onNodesChange={onNodesChange}
        onEdgesChange={onEdgesChange}
        onNodeClick={onNodeClick}
        nodeTypes={nodeTypes}
        edgeTypes={edgeTypes}
        fitView
        fitViewOptions={{ padding: 0.2 }}
        proOptions={{ hideAttribution: true }}
      >
        <Background />
        <Controls />
      </ReactFlow>
    </div>
  );
}
