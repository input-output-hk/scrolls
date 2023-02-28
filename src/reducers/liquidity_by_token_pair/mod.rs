use lazy_static::__Deref;
use pallas::ledger::{
    primitives::babbage::PlutusData,
    traverse::{Asset, MultiEraBlock, MultiEraOutput, MultiEraTx},
};
use serde::Deserialize;

pub mod minswap;
pub mod model;
pub mod sundaeswap;
pub mod utils;
pub mod wingriders;

use crate::{crosscut, prelude::*};

use self::{
    model::{LiquidityPoolDatum, PoolAsset, TokenPair},
    sundaeswap::SundaePoolDatum,
    utils::{build_key_value_pair, contains_currency_symbol, resolve_datum},
    wingriders::WingriderPoolDatum,
};

#[derive(Deserialize)]
pub struct Config {
    pub pool_prefix: Option<String>,
    pub dex_prefix: Option<String>,
    pub pool_currency_symbol: String,
}

pub struct Reducer {
    config: Config,
    policy: crosscut::policies::RuntimePolicy,
}

fn get_asset_amount(asset: &PoolAsset, assets: &Vec<Asset>) -> Option<u64> {
    match asset {
        PoolAsset::Ada => {
            for asset in assets {
                if let Asset::Ada(lovelace_amount) = asset {
                    return Some(*lovelace_amount);
                }
            }
        }
        PoolAsset::AssetClass(matched_currency_symbol_hash, matched_token_name_bytes) => {
            let currency_symbol: String =
                hex::encode(matched_currency_symbol_hash.deref().to_vec());
            let token_name: String = hex::encode(matched_token_name_bytes.deref());
            for asset in assets {
                if let Asset::NativeAsset(currency_symbol_hash, token_name_vector, amount) = asset {
                    if hex::encode(currency_symbol_hash.deref()).eq(&currency_symbol)
                        && hex::encode(token_name_vector).eq(&token_name)
                    {
                        return Some(*amount);
                    }
                }
            }
        }
    }

    None
}

impl Reducer {
    fn get_key_value_pair(
        &self,
        tx: &MultiEraTx,
        utxo: &MultiEraOutput,
    ) -> Result<(String, String), ()> {
        if !contains_currency_symbol(&self.config.pool_currency_symbol, &utxo.non_ada_assets()) {
            return Err(());
        }

        // Get embedded datum for txIns or inline datums if applicable
        let plutus_data: PlutusData = resolve_datum(utxo, tx)?;
        // Try to decode datum as known liquidity pool datum
        let pool_datum = LiquidityPoolDatum::try_from(&plutus_data)?;
        let assets: Vec<Asset> = utxo.assets();
        match pool_datum {
            LiquidityPoolDatum::Minswap(TokenPair { a, b })
            | LiquidityPoolDatum::Wingriders(WingriderPoolDatum { a, b }) => {
                let a_amount_opt: Option<u64> = get_asset_amount(&a, &assets);
                let b_amount_opt: Option<u64> = get_asset_amount(&b, &assets);
                return build_key_value_pair(
                    &TokenPair { a, b },
                    &self.config.dex_prefix,
                    a_amount_opt,
                    b_amount_opt,
                    None,
                )
                .ok_or(());
            }
            LiquidityPoolDatum::Sundaeswap(SundaePoolDatum { a, b, fee }) => {
                let a_amount_opt: Option<u64> = get_asset_amount(&a, &assets);
                let b_amount_opt: Option<u64> = get_asset_amount(&b, &assets);
                return build_key_value_pair(
                    &TokenPair { a, b },
                    &self.config.dex_prefix,
                    a_amount_opt,
                    b_amount_opt,
                    Some(fee),
                )
                .ok_or(());
            }
        };
    }

    pub fn reduce_block<'b>(
        &mut self,
        block: &'b MultiEraBlock<'b>,
        ctx: &crate::model::BlockContext,
        output: &mut super::OutputPort,
    ) -> Result<(), gasket::error::Error> {
        let pool_prefix: Option<&str> = self.config.pool_prefix.as_deref();
        for tx in block.txs().into_iter() {
            for consumed in tx.consumes().iter().map(|i| i.output_ref()) {
                if let Some(Some(utxo)) = ctx.find_utxo(&consumed).apply_policy(&self.policy).ok() {
                    if let Some((k, v)) = self.get_key_value_pair(&tx, &utxo).ok() {
                        output.send(
                            crate::model::CRDTCommand::set_remove(pool_prefix, &k, v).into(),
                        )?;
                    }
                }
            }

            for (_, produced) in tx.produces() {
                if let Some((k, v)) = self.get_key_value_pair(&tx, &produced).ok() {
                    output.send(
                        crate::model::CRDTCommand::set_add(pool_prefix, &k.as_str(), v).into(),
                    )?;
                }
            }
        }

        Ok(())
    }
}

impl Config {
    pub fn plugin(self, policy: &crosscut::policies::RuntimePolicy) -> super::Reducer {
        let reducer = Reducer {
            config: self,
            policy: policy.clone(),
        };
        super::Reducer::LiquidityByTokenPair(reducer)
    }
}
