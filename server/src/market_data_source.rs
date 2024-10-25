use arrayvec::ArrayVec;
use async_trait::async_trait;
use enum_dispatch::enum_dispatch;
use num_derive::FromPrimitive;
use orderbook::Level;
use serde::{de, de::IgnoredAny, de::SeqAccess, Deserialize, Deserializer};
use serde_json::Value;
use std::fmt;
use tokio::sync::mpsc::Sender;
use variant_count::VariantCount;

use crate::orderbook;

pub const DEFAULT_DEPTH: usize = 10;

#[derive(VariantCount, Debug, FromPrimitive, Copy, Clone, PartialEq)]
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
    pub bids: ArrayVec<MarketDatSourceLevel, DEFAULT_DEPTH>,
    pub asks: ArrayVec<MarketDatSourceLevel, DEFAULT_DEPTH>,
}

impl OrderBook {
    pub fn new() -> OrderBook {
        OrderBook {
            bids: ArrayVec::<MarketDatSourceLevel, DEFAULT_DEPTH>::new(),
            asks: ArrayVec::<MarketDatSourceLevel, DEFAULT_DEPTH>::new(),
        }
    }
}

#[derive(Debug, Deserialize, Copy, Clone, PartialEq)]
pub struct MarketDatSourceLevel {
    #[serde(deserialize_with = "de_f64_or_string_as_f64")]
    pub price: f64,
    #[serde(deserialize_with = "de_f64_or_string_as_f64")]
    pub amount: f64,
}

impl MarketDatSourceLevel {
    pub fn to_orderbook_level(&self, exchange: String) -> Level {
        Level {
            amount: self.amount,
            price: self.price,
            exchange,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct MarketDataSourceData {
    #[serde(deserialize_with = "deserialie_vector")]
    pub bids: ArrayVec<MarketDatSourceLevel, DEFAULT_DEPTH>,
    #[serde(deserialize_with = "deserialie_vector")]
    pub asks: ArrayVec<MarketDatSourceLevel, DEFAULT_DEPTH>,
}

fn deserialie_vector<'de, D>(
    deserializer: D,
) -> Result<ArrayVec<MarketDatSourceLevel, DEFAULT_DEPTH>, D::Error>
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

            while let (false, Some(elem)) =
                (elems.is_full(), seq.next_element::<MarketDatSourceLevel>()?)
            {
                elems.push(elem);
            }

            // Consume remaining elements, doesn't work if we don't consume all
            while seq.next_element::<IgnoredAny>()?.is_some() {}
            Ok(elems)
        }
    }
    deserializer.deserialize_seq(SeqVisitor {})
}

pub fn de_f64_or_string_as_f64<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<f64, D::Error> {
    Ok(match Value::deserialize(deserializer)? {
        Value::String(s) => s.parse().map_err(de::Error::custom)?,
        Value::Number(num) => num
            .as_f64()
            .ok_or_else(|| de::Error::custom("Invalid number"))?,
        _ => return Err(de::Error::custom("wrong type")),
    })
}

#[derive(Debug)]
pub struct OrderBookSnap {
    pub exchange: Exchange,
    pub order_book: OrderBook,
}

impl OrderBookSnap {
    pub fn new(exchange: Exchange) -> OrderBookSnap {
        OrderBookSnap {
            exchange,
            order_book: OrderBook::new(),
        }
    }
}

#[derive(Clone)]
pub struct MarketDataSourceInfo {
    pub address: &'static str,
    pub currency: String,
    pub sender: Sender<OrderBookSnap>,
    pub name: &'static str,
}

#[async_trait]
#[enum_dispatch]
pub trait MarketDataSource {
    async fn run(&self);
    fn normalize(&self, msg: &str) -> Result<OrderBookSnap, String>;
}
