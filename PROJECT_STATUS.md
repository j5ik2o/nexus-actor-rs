# Nexus Actor RS - Project Status

Last Updated: 2025/05/28

## Overview

This document consolidates all project planning, issues, and progress tracking into a single source of truth.

## Current Status: Circular Dependency Resolution ✅

### Completed Work
The circular dependency between core and context modules has been successfully resolved using a phased approach:

1. **Phase 1-4 Completed**
   - Extracted independent message types to `core_types` module
   - Resolved circular dependencies by splitting actor/context responsibilities
   - Implemented adapter layer for backward compatibility（2025/09/26 に撤去済み）
   - BaseActor ブリッジを撤去し、循環参照を完全排除

> ❗ 今後は BaseActor 系 API を段階的に撤去する方針に切り替え済み

2. **BaseContext Extensions (2025/05/28)**
   - 当時は `BaseContextExt` や BaseActor 向けヘルパを導入
   - 2025/09/26 時点で BaseActor 系 API を削除済み。以降は Actor トレイトのみを利用

3. **Key Achievements**
   - ✅ All 85+ tests passing
   - ✅ Zero breaking changes to既存 Actor API
   - ✅ BaseActor / BaseContext / MigrationHelpers / ContextAdapter を削除し、Actor トレイトへ統一

### Architecture Improvement
```
Before: actor ←→ context (circular dependency)
After:  core_types → Actor (Props/Context) → Modules (dispatch, supervisor, etc.)
```

## High Priority Issues

### 1. ~~Circular Dependencies~~ ✅ RESOLVED
- Successfully resolved through the creation of independent base traits and adapter layer

### 2. Test Code Organization (In Progress)
**Current Issues:**
- Test code mixed with implementation (`*_test.rs` files)
- Inconsistent test file placement
- Unit and integration tests not clearly separated

**Impact:**
- Reduced test maintainability
- Difficult to navigate test suite
- Unclear test coverage

**Next Steps:**
- **Unit tests**: MUST stay with implementation (e.g., `foo.rs` and `foo/tests.rs`) - DO NOT move to `tests/`
- **Integration tests**: Move to `tests/` directory ONLY
- Remove `*_test.rs` files and reorganize as proper test modules alongside implementation
- Establish clear test organization conventions per Rust 2018 style
- IMPORTANT: Only integration tests that test multiple modules together go in `tests/`

### 3. Module Structure Complexity
**Current Issues:**
- Complex nested directory structure in `actor/`
- Unclear module boundaries
- Difficult navigation for new developers

**Planned Solution:**
- Flatten module structure where appropriate
- Create clear module boundaries
- Improve documentation of module responsibilities

## Future Roadmap

### Phase 5: Complete Migration (Future)
1. ドキュメント更新（BaseActor 記述の削除・Actor 統一方針の明文化）
2. API 名称変更（`spawn_base_actor*` の再命名と段階的非推奨）
3. すべての examples を Actor トレイトベースへ統一（legacy ディレクトリ整理）
4. Performance optimization
5. Public API リリースノート更新（Breaking change の告知）

### Long-term Goals
1. **Simplify Module Structure**
   - Reduce nesting levels
   - Clear separation of concerns
   - Better discoverability

2. **Improve Developer Experience**
   - Better documentation
   - More examples
   - Clear migration guides

3. **Performance Optimization**
   - Reduce dynamic dispatch where possible
   - Optimize message passing
   - Improve actor spawn performance

## Development Guidelines

### For New Code
- `Actor` トレイトのみを使用
- 既存 examples（Actor ベース）を参照
- Props は `Props::from_async_actor_producer` を利用

### For Existing Code
- BaseActor 系実装は削除済み。Actor トレイトへ移行済みであることを確認
- 旧 MigrationHelpers / ActorBridge を利用していたコードは Props + Actor トレイトへ書き換え
- Props ベースのアクターは Actor トレイトで統一

### Testing Guidelines
- **Unit tests**: Place alongside implementation (e.g., `foo/tests.rs`)
- **Integration tests**: Place in `tests/` directory only
- **No `mod.rs` files**: Use Rust 2018 module style
- **Test naming**: Use descriptive test names that explain what is being tested

## Technical Debt Items

1. **Remove unused imports** - Multiple warnings in various modules
2. **Clean up test modules** - Remove unused test utilities
3. **Update documentation** - BaseActor 廃止を反映
4. ~~BaseActor 依存コードの撤去~~ ✅ 完了

## Project Structure

```
nexus-actor-rs/
├── core/                      # Core actor implementation
│   ├── src/actor/
│   │   ├── core_types/       # Independent types (Actor-only)
│   │   ├── core/             # Actor implementation (no BaseActor)
│   │   └── context/          # Context implementation
│   └── examples/
│       ├── actor-advanced-migration/   # Actor traitへの移行デモ
│       ├── actor-base-traits/          # Legacy (要整理)
│       ├── actor-migration/            # Legacy (要整理)
│       └── actor-dual-support/         # Legacy (要整理)
├── remote/                    # Remote actor support
├── cluster/                   # Cluster support
└── utils/                     # Utility modules
```

## References

- Original circular dependency analysis: Completed and documented
- Test organization: Unit tests stay with code, only integration tests go to `tests/`

---

This document supersedes:
- `priority_issues.md`
- `refactoring_action_plan.md`
- `refactoring_todos.md`

All future project status updates should be made to this file.
