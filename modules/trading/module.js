/**
 * Trading Module
 * Logs trading playbooks, setups, performance journaling, and equity curves.
 * READ-ONLY constraints are enforced in broker-guard.
 */
osRegisterModule({
  id: "trading",
  name: "Trading setups",
  icon: "TrendingUp",
  color: "var(--neo-yellow)",
  num: 4,
  version: "1.0.0",

  entityTypes: {
    trade: {
      label: "Trade Logs",
      plural: "Trade Journal",
      icon: "DollarSign",
      attrs: {
        symbol: { type: "text", required: true },
        side: { type: "enum", enum: ["buy", "sell"], required: true },
        entry: { type: "number", required: true },
        exit: { type: "number", required: false },
        stop: { type: "number", required: true },
        target: { type: "number", required: true },
        qty: { type: "number", required: true },
        r_multiple: { type: "number", required: false },
        pnl: { type: "number", required: false },
        emotion: { type: "text", required: false }
      },
      display: {
        title: "symbol",
        subtitle: "pnl",
        badge: "side"
      },
      lifecycle: ["planned", "active", "closed"]
    }
  },

  views: [
    { id: "journal", label: "Trade Journal", kind: "table", type: "trade" },
    { id: "equity_curve", label: "Equity Performance", kind: "calendar", type: "trade" }
  ],

  events: ["trade.planned", "trade.closed", "setup.defined"],

  botCommands: [
    { cmd: "buy", help: "Plan a trade: /buy <symbol> <entry> <stop> <target>", handler: "trade_plan" },
    { cmd: "pnl", help: "View your current performance PnL", handler: "trade_pnl" }
  ],

  agentTools: [
    { name: "trade.log_plan", schema: {}, impl: "log_plan", gated: false },
    { name: "trade.pnl", schema: {}, impl: "pnl", gated: false }
  ]
});
