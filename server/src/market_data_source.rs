use async_trait::async_trait;
use tokio::sync::mpsc::Sender;
use crate::orderbook::Level;
use serde_json::{Value};
use serde::{de, Deserialize, Deserializer, de::SeqAccess, de::IgnoredAny};
use std::fmt;
use arrayvec::ArrayVec;
use variant_count::VariantCount;
use num_derive::FromPrimitive;    

pub const DEFAULT_DEPTH: usize = 10;

#[derive(VariantCount, Debug, FromPrimitive, Copy, Clone)]
pub enum Exchange {
    Binance = 0,
    Bitstamp = 1,
}

impl fmt::Display for Exchange {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Exchange::Binance => write!(f, "binance"),
            Exchange::Bitstamp => write!(f, "bitstamp"),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct OrderBook {
    pub bids: ArrayVec<Level, DEFAULT_DEPTH>,
    pub asks: ArrayVec<Level, DEFAULT_DEPTH>,
}

impl OrderBook {

    pub fn new() -> OrderBook{
        OrderBook {bids: ArrayVec::<Level, DEFAULT_DEPTH>::new(), asks: ArrayVec::<Level, DEFAULT_DEPTH>::new()}
    }

    pub fn add_bid(&mut self, level: Level)
    {
        self.bids.push(level);
    }
    pub fn add_ask(&mut self, level: Level)
    {
        self.asks.push(level);
    }

}

#[derive(Debug, Deserialize, Copy, Clone)]
pub struct MarketDatSourceLevel {
    #[serde(deserialize_with  = "de_f64_or_string_as_f64")]
    pub price: f64,
    #[serde(deserialize_with  = "de_f64_or_string_as_f64")]
    pub amount: f64,
}

#[derive(Debug, Deserialize)]
pub struct MarketDataSourceData {
    #[serde(deserialize_with  = "deserialie_vector")]
    pub bids: ArrayVec<MarketDatSourceLevel, DEFAULT_DEPTH>,
    #[serde(deserialize_with  = "deserialie_vector")]
    pub asks: ArrayVec<MarketDatSourceLevel, DEFAULT_DEPTH>,
}

/*struct WrapperVector<T>(Vec<T>);

impl<T> Into<Vec<T>> for WrapperVector<T> {
    fn into(self) -> Vec<T> {
        self.0
    }
}*/

fn deserialie_vector<'de, D>(deserializer: D) -> Result<ArrayVec<MarketDatSourceLevel, DEFAULT_DEPTH>, D::Error>
where
    D: Deserializer<'de>,
{
    struct SeqVisitor;
    impl<'de> de::Visitor<'de> for SeqVisitor {
        type Value = ArrayVec<MarketDatSourceLevel, DEFAULT_DEPTH>;
        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("BitstampData")
        }
        fn visit_seq<V>(self, mut seq: V) -> Result<Self::Value, V::Error>
        where
            V: SeqAccess<'de>,
        {
            let mut elems = ArrayVec::<MarketDatSourceLevel, DEFAULT_DEPTH>::new();
            
            while let (false, Some(elem)) = (elems.is_full(), seq.next_element::<MarketDatSourceLevel>()?) {
                elems.push(elem);
            }

            // Consume remaining elements
            while seq.next_element::<IgnoredAny>()?.is_some() {}
            Ok(elems)
        }
    }
    deserializer.deserialize_seq(SeqVisitor{})
}

pub fn de_f64_or_string_as_f64<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
    
    Ok(match Value::deserialize(deserializer)? {
        Value::String(s) => s.parse().map_err(de::Error::custom)?,
        Value::Number(num) => num.as_f64().ok_or_else(|| de::Error::custom("Invalid number"))?,
        _ => return Err(de::Error::custom("wrong type")),
    })
}


#[derive(Debug)]
pub struct OrderBookSnap {
    pub exchange: Exchange,
    pub order_book: OrderBook,
}

impl OrderBookSnap {
    pub fn new(exchange: Exchange) -> OrderBookSnap{
        OrderBookSnap{ exchange, 
            order_book: OrderBook::new ()
        }
    }
}

pub struct MarketDataSourceInfo {
    pub address: String,
    pub currency: String,
    pub sender: Sender<OrderBookSnap>,
    pub name: String,
}

#[async_trait]
pub trait MarketDataSource {
    async fn run(&self);
    fn normalize(&self, msg: &str) -> Result<OrderBookSnap, String>;
}