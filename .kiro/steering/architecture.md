# Architectural Steering: shell-doctor

## Philosophy
`shell-doctor` focuses on dynamic runtime diagnostics and millisecond profiling rather than full AST parsing.

## Design Rules
1. **Dynamic Runtime Trace:** Spawn headless shells with high-precision timestamping to capture actual execution time.
2. **Pure Functional Core:** Trace parsing and metric calculations must be deterministic pure functions separated from process spawning and disk I/O.
3. **Property-Based Testing:** All string parsers must be hardened with property-based testing (`proptest`) against arbitrary fuzz inputs.