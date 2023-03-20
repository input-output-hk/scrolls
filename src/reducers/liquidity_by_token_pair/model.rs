use lazy_static::__Deref;
use pallas::{
    crypto::hash::Hash,
    ledger::primitives::babbage::{AssetName, PlutusData},
};
use std::{fmt, str::FromStr};

use super::{
    minswap::MinSwapPoolDatum, sundaeswap::SundaePoolDatum, wingriders::WingriderPoolDatum,
};

pub enum LiquidityPoolDatum {
    Minswap(MinSwapPoolDatum),
    Sundaeswap(SundaePoolDatum),
    Wingriders(WingriderPoolDatum),
}

impl TryFrom<&PlutusData> for LiquidityPoolDatum {
    type Error = ();

    fn try_from(value: &PlutusData) -> Result<Self, Self::Error> {
        if let Some(minswap_token_pair) = MinSwapPoolDatum::try_from(value).ok() {
            return Ok(LiquidityPoolDatum::Minswap(minswap_token_pair));
        } else if let Some(sundae_token_pair) = SundaePoolDatum::try_from(value).ok() {
            return Ok(LiquidityPoolDatum::Sundaeswap(sundae_token_pair));
        } else if let Some(wingriders_token_pair) = WingriderPoolDatum::try_from(value).ok() {
            return Ok(LiquidityPoolDatum::Wingriders(wingriders_token_pair));
        }

        Err(())
    }
}

#[derive(Clone, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub struct TokenPair {
    pub a: PoolAsset,
    pub b: PoolAsset,
}

impl TokenPair {
    pub fn key(&self) -> Option<String> {
        match (&self.a, &self.b) {
            (PoolAsset::Ada, PoolAsset::AssetClass(currency_symbol_1, token_name_1))
            | (PoolAsset::AssetClass(currency_symbol_1, token_name_1), PoolAsset::Ada) => {
                Some(format!(
                    "_._:_{}.{}_",
                    hex::encode(currency_symbol_1.to_vec()),
                    hex::encode(token_name_1.to_vec())
                ))
            }
            (
                PoolAsset::AssetClass(currency_symbol_1, token_name_1),
                PoolAsset::AssetClass(currency_symbol_2, token_name_2),
            ) => {
                let asset_id_1 = format!(
                    "_{}.{}_",
                    hex::encode(currency_symbol_1.to_vec()),
                    hex::encode(token_name_1.to_vec())
                );
                let asset_id_2 = format!(
                    "_{}.{}_",
                    hex::encode(currency_symbol_2.to_vec()),
                    hex::encode(token_name_2.to_vec())
                );

                match asset_id_1.cmp(&asset_id_2) {
                    std::cmp::Ordering::Less => Some(format!("{}:{}", asset_id_1, asset_id_2,)),
                    std::cmp::Ordering::Greater => Some(format!("{}:{}", asset_id_2, asset_id_1,)),
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

impl TryFrom<&PlutusData> for TokenPair {
    type Error = ();

    fn try_from(value: &PlutusData) -> Result<Self, Self::Error> {
        match value {
            PlutusData::Constr(pd) => {
                let _pd1 = pd.fields.get(0).ok_or(())?;
                let _pd2 = pd.fields.get(1).ok_or(())?;

                return match (
                    PoolAsset::try_from(_pd1).ok(),
                    PoolAsset::try_from(_pd2).ok(),
                ) {
                    (Some(a), Some(b)) => Ok(Self { a, b }),
                    _ => Err(()),
                };
            }
            _ => Err(()),
        }
    }
}

pub type CurrencySymbol = Hash<28>;

pub fn currency_symbol_from(str: &Vec<u8>) -> Option<CurrencySymbol> {
    Hash::from_str(hex::encode(str.clone()).as_str()).ok()
}

#[derive(Clone, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum PoolAsset {
    Ada,
    AssetClass(CurrencySymbol, AssetName),
}

impl TryFrom<&PlutusData> for PoolAsset {
    type Error = ();

    fn try_from(value: &PlutusData) -> Result<Self, Self::Error> {
        if let PlutusData::Constr(pd) = value {
            return match (pd.fields.get(0), pd.fields.get(1)) {
                (
                    Some(PlutusData::BoundedBytes(currency_symbol)),
                    Some(PlutusData::BoundedBytes(token_name)),
                ) => {
                    if currency_symbol.len() == 0 && token_name.len() == 0 {
                        return Ok(PoolAsset::Ada);
                    } else if let Some(pid) = currency_symbol_from(&currency_symbol.clone().deref())
                    {
                        return Ok(PoolAsset::AssetClass(
                            pid,
                            AssetName::from(token_name.to_vec()),
                        ));
                    }

                    Err(())
                }
                _ => Err(()),
            };
        }

        Err(())
    }
}

impl std::fmt::Display for PoolAsset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.clone() {
            PoolAsset::Ada => write!(f, ""),
            PoolAsset::AssetClass(currency_symbol, token_name) => {
                write!(
                    f,
                    "{}.{}",
                    hex::encode(currency_symbol.to_vec()),
                    hex::encode(token_name.to_vec())
                )
            }
        }
    }
}

#[derive(core::cmp::PartialOrd, Debug)]
pub struct AssetClass {
    currency_symbol: CurrencySymbol,
    asset_name: AssetName,
}

impl std::fmt::Display for AssetClass {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "AssetClass {{ policy: '{}', name: '{}' }}",
            hex::encode(self.currency_symbol.to_vec()),
            hex::encode(self.asset_name.to_vec())
        )
    }
}

impl PartialEq for AssetClass {
    fn eq(&self, other: &Self) -> bool {
        self.currency_symbol == other.currency_symbol && self.asset_name == other.asset_name
    }
}
