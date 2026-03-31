use markets::{Ticker, Interval, TimeRange};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ticker = Ticker::new("AAPL").await?;

    let chart = ticker.chart(Interval::OneDay, TimeRange::Max).await?;

    println!("AAPL Historical Data (1d interval, max range)");
    println!("Total candles: {}", chart.candles.len());
    println!("---");

    for candle in &chart.candles {
        let date = chrono::DateTime::from_timestamp(candle.timestamp, 0)
            .map(|dt| dt.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| candle.timestamp.to_string());

        println!(
            "{date}  O: {:.2}  H: {:.2}  L: {:.2}  C: {:.2}  V: {}",
            candle.open, candle.high, candle.low, candle.close, candle.volume
        );
    }

    Ok(())
}
