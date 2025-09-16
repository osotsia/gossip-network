/**
 * JSDoc type definitions for data structures from the Rust backend.
 * This provides type-hinting and autocompletion in IDEs.
 */

/**
 * A unique identifier for a node, represented as a hex string.
 * @typedef {string} NodeId
 */

/**
 * @typedef {object} TelemetryData
 * @property {number} timestamp_ms
 * @property {number} value
 */

/**
 * @typedef {object} NodeInfo
 * @property {TelemetryData} telemetry
 * @property {number} community_id
 */

/**
 * The full network state as seen by one node.
 * The `nodes` property is a map from NodeId to NodeInfo.
 * @typedef {object} NetworkState
 * @property {NodeId | null} self_id
 * @property {Object.<NodeId, NodeInfo>} nodes
 * @property {NodeId[]} edges
 */

/**
 * @typedef {'connecting' | 'connected' | 'disconnected'} ConnectionStatus
 */

export {};