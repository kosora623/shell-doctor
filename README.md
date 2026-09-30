# 🩺 shell-doctor

Lightning-fast shell runtime doctor written in Rust, built with Kiro.

## 🚀 Kiro University Challenge Implementation Matrix

| Lesson | Focus Area | Implementation Path |
| :--- | :--- | :--- |
| **Lesson 1** | Spec-driven dev (EARS) | `.kiro/specs/trace-parser-spec.md` |
| **Lesson 2** | Steering documents | `.kiro/steering/architecture.md` |
| **Lesson 3** | Automation Hooks | `.kiro/hooks/hooks.json` |
| **Lesson 4** | Property-Based Testing | `src/parser.rs` (tested with `proptest`) |
| **Lesson 5** | Powers | `.kiro/powers/profile-analyzer.json` |
| **Lesson 6** | MCP Configuration | `.kiro/mcp.json` |
| **Lesson 7** | Custom Agents | `.kiro/agents/profile-optimizer.json` |

## 🛠️ Usage

```bash
# Run property-based tests (Lesson 4)
cargo test

# Run trace bottleneck diagnostic demo
cargo run -- demo