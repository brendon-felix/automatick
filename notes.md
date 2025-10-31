# Notes

## TUI Architecture Diagram

```
┌─────────────────────────────────────────────────────────┐
│                        main.rs                          │
│  - Entry point                                          │
│  - Authentication flow                                  │
│  - Client initialization                                │
└───────────────────────────┬─────────────────────────────┘
                            │
                            ↓
┌─────────────────────────────────────────────────────────┐
│                          App                            │
│  - Main event loop                                      │
│  - State management                                     │
│  - Mode handling (Normal/Insert/Processing/Help)        │
│  - Task cache                                           │
│  - Arc<TickTick> client                                 │
└─────────┬─────────────────┬─────────────────┬───────────┘
          │                 │                 │
          ↓                 ↓                 ↓
      ┌─────────┐      ┌──────────┐      ┌──────────┐
      │   TUI   │      │    UI    │      │  Action  │
      │         │      │          │      │          │
      │ Event   │      │ Render   │      │ Action   │
      │ Stream  │      │ Layout   │      │ Types    │
      └─────────┘      └──────────┘      └──────────┘
```

## Async Design

### Why Arc<TickTick>?
The TickTick client doesn't implement `Clone`, but we need to share it across async tasks spawned from the event loop. We use `Arc<TickTick>` to enable safe, thread-safe sharing.

```rust
pub struct App {
    client: Arc<TickTick>,  // Shared across async tasks
    // ...
}

// Usage in async spawn:
let client = Arc::clone(&self.client);
tokio::spawn(async move {
    fetch_tasks(&client).await
});
```

## Future Enhancements

Potential improvements:
- [ ] Task editing (change title, priority, description)
- [ ] Multi-project support
- [ ] Task filtering and search
- [ ] Sorting options
- [ ] Bulk operations
- [ ] Keyboard shortcuts customization
- [ ] Color theme customization
- [ ] Task subtasks view
- [ ] Due date management
- [ ] Tag management UI
