# Doge Roadmap: Full pixi_command_dispatcher Replacement

This document outlines the development roadmap for evolving Doge into a complete replacement for pixi_command_dispatcher.

## Current Status

Doge currently provides:
- ✅ Basic async task execution with tokio
- ✅ DAG-based dependency management using daggy
- ✅ Task deduplication (basic level)
- ✅ Parallel execution with dependency respect
- ✅ Progress reporting system
- ✅ Dependency data flow via TaskContext
- ✅ Integration with pixi-style reporters

## Gap Analysis: What's Missing for Full Replacement

### 🚨 Critical Components (Must-Have)

#### 1. **Type-Safe Task Specification System**
**Current pixi_command_dispatcher:**
```rust
pub trait TaskSpec {
    type Output: Send + Sync + 'static;
    type Error: Send + Sync + 'static;
}

pub struct Task<T: TaskSpec> {
    spec: T,
    parent: Option<TaskId>,
    result_sender: oneshot::Sender<Result<T::Output, T::Error>>,
}
```

**What we need to implement:**
- TaskSpec trait with associated types
- Task wrapper with result channels
- Type-safe task composition patterns
- Parent task tracking for hierarchical execution

#### 2. **Domain-Specific Task Types**
**Missing task specifications:**
- `SolvePixiEnvironmentSpec` - Complex environment solving with source/binary splitting
- `SolveCondaEnvironmentSpec` - Pure conda environment resolution
- `InstallPixiEnvironmentSpec` - Package installation orchestration
- `SourceMetadataSpec` - Source package metadata extraction
- `SourceBuildSpec` - Source package compilation
- `BuildBackendMetadataSpec` - Build backend metadata retrieval
- `SourceBuildCacheStatusSpec` - Build cache validation
- `InstantiateToolEnvironmentSpec` - Tool environment creation
- `InstantiateBackendSpec` - Build backend instantiation

#### 3. **Advanced Task Deduplication**
**Current pixi_command_dispatcher:**
```rust
pub struct PendingDeduplicatingTask<T> {
    state: PendingTaskState<T>,
    waiters: Vec<oneshot::Sender<Result<T, CommandDispatcherError<T>>>>,
}
```

**What we need:**
- Waiter management for duplicate tasks
- State tracking (Pending/Result/Errored)
- Result broadcasting to multiple waiters
- Proper cleanup on cancellation

#### 4. **Background Thread Architecture**
**Current pixi architecture:**
- Dedicated background thread with tokio runtime
- Message passing via unbounded channels
- Graceful shutdown handling
- Weak reference support for lifecycle management

**What we need to implement:**
- Background thread spawning
- ForegroundMessage enum for command dispatch
- Proper shutdown coordination
- Channel lifecycle management

### 🔴 High Priority Components

#### 5. **Pixi Ecosystem Integration**
**Required integrations:**
- **Rattler Gateway**: Conda repodata fetching and caching
- **PackageCache**: Conda package storage and retrieval
- **GitResolver**: Git repository management and caching
- **Source Checkouts**: Path/Git/URL source handling
- **Virtual Packages**: Platform-specific package detection
- **Channel Configuration**: Complex conda channel management

#### 6. **Advanced Configuration System**
**CommandDispatcherBuilder pattern with:**
- Cache directory configuration
- Concurrency limits (per operation type)
- Gateway configuration
- Executor mode (Concurrent/Serial)
- Build environment settings
- Backend overrides
- Reporter configuration

#### 7. **Sophisticated Error Handling**
**Required error infrastructure:**
- `CommandDispatcherError<E>` enum
- Domain-specific error types
- Miette integration for diagnostics
- Error conversion traits
- Cycle detection errors
- Detailed error context

### 🟡 Medium Priority Components

#### 8. **Performance Optimizations**
- Multi-level caching (source metadata, build cache, package cache)
- Streaming output for long-running tasks
- Resource-aware scheduling
- Memory-efficient task queuing
- Lazy task initialization

#### 9. **Testing Infrastructure**
- EventReporter for comprehensive testing
- Deterministic serial executor
- Integration test helpers
- Snapshot testing support
- Mock implementations for external dependencies

#### 10. **Platform-Specific Features**
- Tool platform normalization (WinArm64 → Win64)
- Cross-platform path handling
- Link script execution
- Home directory resolution
- Security sandboxing considerations

## Implementation Roadmap

### Phase 1: Core Architecture (3-4 weeks)

**Week 1-2: Task Specification System**
- [ ] Implement TaskSpec trait with associated types
- [ ] Create Task wrapper with result channels
- [ ] Build message passing infrastructure
- [ ] Add parent task tracking

**Week 3: Background Thread Architecture**
- [ ] Implement background thread spawning
- [ ] Create ForegroundMessage enum
- [ ] Build command dispatcher core
- [ ] Add graceful shutdown

**Week 4: Advanced Deduplication**
- [ ] Implement PendingDeduplicatingTask
- [ ] Add waiter management
- [ ] Build result broadcasting
- [ ] Handle cancellation properly

### Phase 2: Domain Integration (4-6 weeks)

**Week 5-6: Core Task Types**
- [ ] Implement SolvePixiEnvironmentSpec
- [ ] Implement InstallPixiEnvironmentSpec
- [ ] Add basic conda integration

**Week 7-8: Build System Tasks**
- [ ] Implement SourceBuildSpec
- [ ] Add BuildBackendMetadataSpec
- [ ] Create source metadata extraction

**Week 9-10: Ecosystem Integration**
- [ ] Integrate with rattler gateway
- [ ] Add package cache support
- [ ] Implement git resolver integration
- [ ] Add source checkout handling

### Phase 3: Production Features (2-3 weeks)

**Week 11: Configuration & Error Handling**
- [ ] Build CommandDispatcherBuilder
- [ ] Implement comprehensive error types
- [ ] Add diagnostic reporting
- [ ] Create configuration validation

**Week 12-13: Performance & Testing**
- [ ] Add multi-level caching
- [ ] Implement streaming output
- [ ] Build testing infrastructure
- [ ] Add integration tests

### Phase 4: Ecosystem Polish (1-2 weeks)

**Week 14: Platform & Polish**
- [ ] Add platform-specific handling
- [ ] Complete reporter integration
- [ ] Write migration guides
- [ ] Performance benchmarking

## Migration Strategy

### Gradual Adoption Path

1. **Start with Simple Workflows**
   - Use Doge for isolated task execution
   - Test with non-critical workflows
   - Gather performance metrics

2. **Incremental Feature Addition**
   - Add pixi-specific features as needed
   - Maintain compatibility layer
   - Run both systems in parallel

3. **Production Rollout**
   - Phase out pixi_command_dispatcher gradually
   - Monitor for regressions
   - Maintain fallback capability

### Compatibility Considerations

- Maintain API compatibility where possible
- Provide migration tools for existing code
- Document breaking changes clearly
- Support legacy reporter interfaces

## Success Metrics

### Performance Goals
- Task execution overhead < 1ms
- Memory usage parity with current system
- Deduplication efficiency > 95%
- Parallel execution scaling to 100+ tasks

### Feature Completeness
- 100% task type coverage
- Full reporter compatibility
- Complete error handling parity
- All platform support maintained

### Code Quality
- Comprehensive test coverage (>90%)
- Documentation for all public APIs
- Example code for common patterns
- Performance benchmarks included

## Alternative Approaches

### Minimal Viable Replacement
If full replacement timeline is too long, consider:
1. **Core Tasks Only**: Implement only critical task types
2. **Simplified Configuration**: Start with minimal options
3. **Basic Integration**: Focus on essential ecosystem components
4. **Incremental Enhancement**: Add features based on usage

### Hybrid Approach
- Use Doge for new workflows
- Keep pixi_command_dispatcher for legacy
- Build compatibility bridge
- Migrate gradually over time

## Conclusion

The full replacement of pixi_command_dispatcher requires approximately **10-15 weeks** of focused development. However, Doge's solid architectural foundation makes it an excellent candidate for this evolution. The modular approach allows for incremental development and testing, reducing risk while building toward full feature parity.

The roadmap prioritizes critical infrastructure first, followed by domain-specific features, ensuring that each phase delivers usable functionality while building toward the complete replacement goal.