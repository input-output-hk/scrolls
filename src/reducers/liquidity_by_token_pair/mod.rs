use pallas::ledger::{
    primitives::babbage::PlutusData,
    traverse::{Asset, MultiEraBlock, MultiEraOutput, MultiEraTx},
};
use serde::Deserialize;

pub mod minswap;
pub mod model;
pub mod muesliswap;
pub mod sundaeswap;
pub mod utils;
pub mod wingriders;

use crate::{crosscut, prelude::*};

use self::{
    minswap::MinSwapPoolDatum,
    model::{LiquidityPoolDatum, TokenPair},
    muesliswap::MuesliSwapPoolDatum,
    sundaeswap::SundaePoolDatum,
    utils::{build_key_value_pair, contains_currency_symbol, get_asset_amount, resolve_datum},
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
            LiquidityPoolDatum::MuesliSwapPoolDatum(MuesliSwapPoolDatum { a, b })
            | LiquidityPoolDatum::Minswap(MinSwapPoolDatum { a, b })
            | LiquidityPoolDatum::Wingriders(WingriderPoolDatum { a, b }) => {
                let a_amount_opt: Option<u64> = get_asset_amount(&a, &assets);
                let b_amount_opt: Option<u64> = get_asset_amount(&b, &assets);
                return build_key_value_pair(
                    &TokenPair { a, b },
                    &self.config.dex_prefix,
                    a_amount_opt,
                    b_amount_opt,
                    None,
                    None,
                )
                .ok_or(());
            }
            LiquidityPoolDatum::Sundaeswap(SundaePoolDatum { a, b, fee, pool_id }) => {
                let a_amount_opt: Option<u64> = get_asset_amount(&a, &assets);
                let b_amount_opt: Option<u64> = get_asset_amount(&b, &assets);
                return build_key_value_pair(
                    &TokenPair { a, b },
                    &self.config.dex_prefix,
                    a_amount_opt,
                    b_amount_opt,
                    Some(fee),
                    Some(pool_id),
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
