# Use cases for the historical 1-min OHLCV dataset

Non-backtesting ideas for the data sitting in `market_data.bars_1m`.

## Personal trading / decision support

- **Personal Bloomberg terminal** — your desktop app already charts; layer on cross-symbol comparison, watchlist heatmaps, sector performance grids, relative-strength rankings. The thing Bloomberg charges $24k/year for is mostly "fast cross-asset queries against complete data." You have that.
- **Pre-market / post-market scanners** — every morning at 9:00 ET, query "which symbols had unusual after-hours volume," "biggest 1-min ranges in pre-market," "stocks within 1% of 52-week highs." Push the list to Slack/Telegram/your phone before the bell. ClickHouse can do this in under a second.
- **Earnings-day playbook** — for any upcoming earnings, pull "how this stock moved 1-min by 1-min on its last 8 earnings days." Build a probabilistic move-distribution chart so you size positions sensibly.
- **Real-time alert engine** — pair the historical data with a streaming layer (Yahoo's quote endpoint or a Polygon subscription). Every 60 sec compute "is this minute's bar > 3σ above the past 60 days' minute-of-day distribution?" → push notification.

## Market microstructure / education

- **Liquidity heatmap by minute-of-day** — for each symbol, plot average volume / spread by minute across the trading day over the past N years. Reveals the open auction spike, mid-day lull, MOC ramp, FOMC pauses. Useful for picking when to actually execute (avoid the worst slippage windows).
- **Day-of-week / month-of-year seasonality** — does AAPL really sell off in Sept? Does the Santa rally exist? Run it on every symbol over 21 years and produce a calendar heatmap. Good blog material; also informs swing-trade timing.
- **Pre-FOMC drift study** — replicate famous papers ("Pre-FOMC announcement drift") on your own data. Education, not strategy.
- **News-event impact studies** — when a stock gaps up >5% on volume, how often does it close above/below the open? How does the path look minute-by-minute? Build the empirical playbook.

## Cross-asset analytics

- **Sector rotation dashboard** — relative-strength of the 11 GICS sectors over 1d / 1w / 1m windows, updated hourly. Useful for top-down trade ideas.
- **Pairs / basket monitoring** — pick a pair (KO/PEP, MSFT/GOOGL), compute the rolling spread z-score continuously. Alert when it hits ±2σ. Doesn't need full backtest infrastructure — just a query + alert.
- **ETF vs basket tracking error** — compare SPY's intraday path vs a synthetic basket of its top 50 holdings. Teaches you how arbitrage actually works in practice. Same pattern works for any sector ETF.
- **Index reconstruction** — recompute the S&P 500 from constituents and compare to actuals. Catches data-quality issues, also lets you build "what if we held the equal-weight version instead" portfolios.

## Quality / monitoring infrastructure

- **Data-quality monitor** — daily job that flags symbols with: gaps in trading hours, volume = 0 for >N min during market hours, OHLC violations (e.g. high < close), suspicious split-day jumps. You'll be amazed how much bad data is in any vendor's feed.
- **Vendor comparison** — pull 1-min bars for AAPL today from FMP, Yahoo, Polygon, Tradier. Compare. Tells you which vendor to trust for what.
- **Anomaly detection cron** — every hour, query "any symbol whose latest minute is >5σ outside its 30-day distribution?" Flag in a Slack channel. Surfaces breaking news / pump-and-dumps before headlines.

## Calling other developers / ML

- **Public data API** — wrap your ClickHouse behind a tiny REST/GraphQL layer with rate limiting, charge $5/month, sell to retail traders. Polygon and Twelvedata each charge 10-100× that. (Niche, but real.)
- **Train a model on raw bars** — small transformer that predicts next 5-min direction. Probably won't be profitable, but you learn a lot about why financial ML is hard. Use the data as a real-world dataset for ML practice.
- **Synthetic data generator** — train a GAN/diffusion model on real bars to generate plausible-looking synthetic data. Useful for stress-testing your backtests against scenarios that didn't happen.
- **LLM RAG over market events** — embed news/SEC filings, retrieve them by date, then query "show me what AAPL did in the 30 min after each 8-K filing in 2024" — natural language → chart. Fun side project, good demo.

## Charting feature ideas (extends what you already have)

- **Volume profile overlay** — distribution of volume by price level for any window. Standard pro-tool feature, trivial query against your data.
- **Volume-weighted average price line** — overlay live VWAP since open. One SQL query.
- **Anchored VWAP from any user-clicked bar** — clicker drops a pin on the chart, VWAP gets recalculated from that point. Killer feature in TradingView Pro.
- **Multi-timeframe coherence** — show 1m + 5m + 15m + 1h + 1d for one symbol simultaneously, synchronized.
- **Heatmap chart** — instead of candles, color each minute by return percentile. Reveals consolidation vs trending phases visually.

## Operational / fun

- **Automated trading journal** — every time you place a trade in your broker, store the entry/exit and overlay your fills on the historical chart. Compute P&L per trade, tag by setup type, learn what works for you personally.
- **Daily market summary email** — automated 7AM email: top movers, volume leaders, sector heatmap, your watchlist's after-hours moves, today's earnings. Replaces 4-5 paid newsletters.
- **Market replay tool** — pick any past day, "play it back" minute-by-minute on your chart with a speed control. Powerful learning tool — train your eye on real historical conditions.
