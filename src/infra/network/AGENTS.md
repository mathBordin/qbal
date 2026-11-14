### The IoEvent pump

`runtime/pump.rs::start_tokio` drains IoEvents serially:

- **Routing**: `route_queue_event` (`infra/queue/dispatch.rs`) runs first and
  owns `AdvanceNativeQueue`/`FinishNativeQueue` plus the transports while the
  native queue slot plays; then `route_qobuz_event` (`infra/qobuz/dispatch.rs`)
  takes the `qobuz:` starts, the Qobuz browse/search/login events, and the
  transports while a Qobuz session owns the sink. An event neither router
  consumes reaches `Network::handle_network_event`, whose catch-all arm ignores
  a transport that arrives while nothing plays.
- **Service lane**: `Network::runs_on_service_lane` lists events that run on a
  detached task (lyrics, cover art, announcements, listening stats, the recap)
  so slow work cannot block the serial pump. Their handlers touch only
  `self.app` and their own HTTP clients.

A new IoEvent needs a router arm or a `handle_network_event` arm, and a
decision about the service lane. A detached task needs its own staleness check
(generation/epoch) before it writes `App`.
