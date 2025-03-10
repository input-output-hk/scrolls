use lazy_static::__Deref;
use pallas::{
    codec::utils::CborWrap,
    ledger::{
        primitives::babbage::{AssetName, DatumOption, PlutusData},
        traverse::{Asset, MultiEraOutput, MultiEraTx, OriginalHash},
    },
};
use serde_json::json;

use super::model::{currency_symbol_from, PoolAsset, TokenPair};

pub fn contains_currency_symbol(currency_symbol: &String, assets: &Vec<Asset>) -> bool {
    assets.iter().any(|asset| {
        asset
            .policy_hex()
            .or(Some(String::new())) // in case ADA is part of the vector
            .unwrap()
            .as_str()
            .eq(currency_symbol.as_str())
    })
}

pub fn pool_asset_from(hex_currency_symbol: &String, hex_asset_name: &String) -> Option<PoolAsset> {
    if hex_currency_symbol.len() == 0 && hex_asset_name.len() == 0 {
        return Some(PoolAsset::Ada);
    }

    if let (Some(pid), Some(tkn)) = (
        hex::decode(hex_currency_symbol).ok(),
        hex::decode(hex_asset_name).ok(),
    ) {
        if let Some(cs) = currency_symbol_from(&pid) {
            return Some(PoolAsset::AssetClass(cs, AssetName::from(tkn)));
        }
    }

    None
}

pub fn resolve_datum(utxo: &MultiEraOutput, tx: &MultiEraTx) -> Result<PlutusData, ()> {
    match utxo.datum() {
        Some(DatumOption::Data(CborWrap(pd))) => Ok(pd),
        Some(DatumOption::Hash(datum_hash)) => {
            for raw_datum in tx.clone().plutus_data() {
                if raw_datum.original_hash().eq(&datum_hash) {
                    return Ok(raw_datum.clone().unwrap());
                }
            }

            Err(())
        }
        _ => Err(()),
    }
}

pub fn serialize_value(
    dex_prefix: &Option<String>,
    a_amount_opt: Option<u64>,
    b_amount_opt: Option<u64>,
    fee_opt: Option<f64>,
    pool_id_opt: Option<String>,
) -> Option<String> {
    let a_amount: u64 = a_amount_opt?;
    let b_amount: u64 = b_amount_opt?;

    let mut result = json!({
        "token_a": a_amount.to_string(),
        "token_b": b_amount.to_string(),
    });

    if let Some(dex_prefix) = dex_prefix {
        result["dex"] = serde_json::Value::String(String::from(dex_prefix.as_str()));
    }

    if let Some(fee) = fee_opt {
        if let Some(n) = serde_json::Number::from_f64(fee) {
            result["fee"] = serde_json::Value::Number(n);
        }
    }

    if let Some(pool_id) = pool_id_opt {
        result["pool_id"] = serde_json::Value::String(String::from(pool_id.as_str()));
    }

    Some(result.to_string())
}

pub fn build_key_value_pair(
    token_pair: &TokenPair,
    dex_prefix: &Option<String>,
    a_amount_opt: Option<u64>,
    b_amount_opt: Option<u64>,
    fee_opt: Option<f64>,
    pool_id_opt: Option<String>,
) -> Option<(String, String)> {
    let value: Option<String> = match (&token_pair.a, &token_pair.b) {
        (PoolAsset::Ada, PoolAsset::AssetClass(_, _)) => {
            serialize_value(dex_prefix, a_amount_opt, b_amount_opt, fee_opt, pool_id_opt)
        }
        (PoolAsset::AssetClass(_, _), PoolAsset::Ada) => {
            serialize_value(
                dex_prefix,
                b_amount_opt, // swapped
                a_amount_opt, // swapped
                fee_opt,
                pool_id_opt,
            )
        }
        (
            PoolAsset::AssetClass(currency_symbol_1, token_name_1),
            PoolAsset::AssetClass(currency_symbol_2, token_name_2),
        ) => {
            let asset_id_1 = format!(
                "{}.{}",
                hex::encode(currency_symbol_1.to_vec()),
                hex::encode(token_name_1.to_vec())
            );
            let asset_id_2 = format!(
                "{}.{}",
                hex::encode(currency_symbol_2.to_vec()),
                hex::encode(token_name_2.to_vec())
            );
            match asset_id_1.cmp(&asset_id_2) {
                std::cmp::Ordering::Less => {
                    serialize_value(dex_prefix, a_amount_opt, b_amount_opt, fee_opt, pool_id_opt)
                }
                std::cmp::Ordering::Greater => serialize_value(
                    dex_prefix,
                    b_amount_opt, // swapped
                    a_amount_opt, // swapped
                    fee_opt,
                    pool_id_opt,
                ),
                _ => None,
            }
        }
        _ => None,
    };

    if let (Some(key), Some(value)) = (token_pair.key(), value) {
        return Some((key, value));
    }
    None
}

pub fn get_asset_amount(asset: &PoolAsset, assets: &Vec<Asset>) -> Option<u64> {
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
                    if hex::encode(currency_symbol_hash.deref().to_vec()).eq(&currency_symbol)
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

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use pallas::ledger::{
        primitives::{babbage::PlutusData, Fragment},
        traverse::Asset,
    };

    use crate::reducers::liquidity_by_token_pair::{
        model::{CurrencySymbol, PoolAsset, TokenPair},
        utils::{
            build_key_value_pair, contains_currency_symbol, get_asset_amount, pool_asset_from,
            serialize_value,
        },
    };

    static CURRENCY_SYMBOL_1: &str = "93744265ed9762d8fa52c4aacacc703aa8c81de9f6d1a59f2299235b";
    static CURRENCY_SYMBOL_2: &str = "158fd94afa7ee07055ccdee0ba68637fe0e700d0e58e8d12eca5be46";

    fn mock_assets() -> Vec<Asset> {
        [
            Asset::NativeAsset(
                CurrencySymbol::from_str(CURRENCY_SYMBOL_1).ok().unwrap(),
                "Tkn1".to_string().as_bytes().to_vec(),
                1,
            ),
            Asset::NativeAsset(
                CurrencySymbol::from_str(CURRENCY_SYMBOL_1).ok().unwrap(),
                "Tkn2".to_string().as_bytes().to_vec(),
                2,
            ),
            Asset::NativeAsset(
                CurrencySymbol::from_str(CURRENCY_SYMBOL_2).ok().unwrap(),
                "Tkn3".to_string().as_bytes().to_vec(),
                3,
            ),
        ]
        .to_vec()
    }

    #[test]
    fn test_contains_currency_symbol() {
        let mock_assets = mock_assets();
        assert_eq!(
            contains_currency_symbol(&CURRENCY_SYMBOL_1.to_string(), &mock_assets),
            true
        );
        assert_eq!(
            contains_currency_symbol(&CURRENCY_SYMBOL_2.to_string(), &mock_assets),
            true
        );
        assert_eq!(
            contains_currency_symbol(&"".to_string(), &mock_assets),
            false
        );
        assert_eq!(
            contains_currency_symbol(&"123abc".to_string(), &mock_assets),
            false
        );
    }

    #[test]
    fn test_valid_key_for_ada_min() {
        let hex_pool_datum = "d8799fd8799f4040ffd8799f581c29d222ce763455e3d7a09a665ce554f00ac89d2e99a1a83d267170c6434d494eff1b00004ce6fb73282200d87a80ff";
        let data = hex::decode(hex_pool_datum).unwrap();
        let plutus_data = PlutusData::decode_fragment(&data).unwrap();
        let token_pair = TokenPair::try_from(&plutus_data).unwrap();
        let key = token_pair.key();
        assert_eq!(true, key.is_some());
        assert_eq!(
            "29d222ce763455e3d7a09a665ce554f00ac89d2e99a1a83d267170c6.4d494e",
            key.unwrap()
        )
    }

    #[test]
    fn test_valid_key_for_min_ada() {
        let hex_pool_datum = "d8799fd8799f4040ffd8799f581c29d222ce763455e3d7a09a665ce554f00ac89d2e99a1a83d267170c6434d494eff1b00004ce6fb73282200d87a80ff";
        let data = hex::decode(hex_pool_datum).unwrap();
        let plutus_data = PlutusData::decode_fragment(&data).unwrap();
        let tp = TokenPair::try_from(&plutus_data).unwrap();
        let token_pair = TokenPair { a: tp.b, b: tp.a };
        let key = token_pair.key();
        assert_eq!(true, key.is_some());
        assert_eq!(
            "29d222ce763455e3d7a09a665ce554f00ac89d2e99a1a83d267170c6.4d494e",
            key.unwrap()
        )
    }

    #[test]
    fn test_valid_key_for_same_assets() {
        let hex_pool_datum = "d8799fd8799f4040ffd8799f581c29d222ce763455e3d7a09a665ce554f00ac89d2e99a1a83d267170c6434d494eff1b00004ce6fb73282200d87a80ff";
        let data = hex::decode(hex_pool_datum).unwrap();
        let plutus_data = PlutusData::decode_fragment(&data).unwrap();
        let tp1 = TokenPair::try_from(&plutus_data).unwrap();
        let tp2 = TokenPair::try_from(&plutus_data).unwrap();

        let tp1_invalid = TokenPair { a: tp1.b, b: tp2.b };
        let key1 = tp1_invalid.key();
        assert_eq!(true, key1.is_none());

        let tp2_invalid = TokenPair { a: tp1.a, b: tp2.a };
        let key2 = tp2_invalid.key();
        assert_eq!(true, key2.is_none());
    }

    #[test]
    fn test_valid_key_for_min_djed() {
        let hex_pool_datum = "d8799fd8799f581c29d222ce763455e3d7a09a665ce554f00ac89d2e99a1a83d267170c6434d494effd8799f581c8db269c3ec630e06ae29f74bc39edd1f87c819f1056206e879a1cd614c446a65644d6963726f555344ff1b000000012d9b96321b000000012dc40542d8799fd8799fd8799fd8799f581caafb1196434cb837fd6f21323ca37b302dff6387e8a84b3fa28faf56ffd8799fd8799fd8799f581c52563c5410bff6a0d43ccebb7c37e1f69f5eb260552521adff33b9c2ffffffffd87a80ffffff";
        let data = hex::decode(hex_pool_datum).unwrap();
        let plutus_data = PlutusData::decode_fragment(&data).unwrap();
        let token_pair = TokenPair::try_from(&plutus_data).unwrap();

        let key = token_pair.key();
        assert_eq!(true, key.is_some());
        assert_eq!("29d222ce763455e3d7a09a665ce554f00ac89d2e99a1a83d267170c6.4d494e:8db269c3ec630e06ae29f74bc39edd1f87c819f1056206e879a1cd61.446a65644d6963726f555344", key.unwrap());

        assert_eq!(
            "29d222ce763455e3d7a09a665ce554f00ac89d2e99a1a83d267170c6.4d494e",
            token_pair.a.to_string()
        );
        assert_eq!(
            "8db269c3ec630e06ae29f74bc39edd1f87c819f1056206e879a1cd61.446a65644d6963726f555344",
            token_pair.b.to_string()
        );

        let member = serialize_value(
            &Some(String::from("min")),
            Some(10),
            Some(20),
            Some(0.005),
            Some(String::from("08")),
        );
        assert_eq!(true, member.is_some());
        assert_eq!(
            "{\"dex\":\"min\",\"fee\":0.005,\"pool_id\":\"08\",\"token_a\":\"10\",\"token_b\":\"20\"}",
            member.unwrap()
        );

        let swapped_token_pair = TokenPair {
            a: token_pair.b.clone(),
            b: token_pair.a.clone(),
        };

        assert_eq!(token_pair.key(), swapped_token_pair.key());
        assert_eq!(
            build_key_value_pair(
                &token_pair,
                &None,
                Some(10),
                Some(20),
                Some(0.005),
                Some(String::from("08"))
            ),
            build_key_value_pair(
                &swapped_token_pair,
                &None,
                Some(20),
                Some(10),
                Some(0.005),
                Some(String::from("08"))
            ),
        );
        assert_eq!(
            build_key_value_pair(
                &token_pair,
                &None,
                Some(10),
                Some(20),
                Some(0.005),
                Some(String::from("08"))
            ),
            build_key_value_pair(
                &swapped_token_pair,
                &None,
                Some(20),
                Some(10),
                Some(0.005),
                Some(String::from("08"))
            ),
        );
    }

    #[test]
    fn test_invalid_key_for_ada_ada() {
        let token_pair = TokenPair {
            a: PoolAsset::Ada,
            b: PoolAsset::Ada,
        };
        let key = token_pair.key();
        assert_eq!(true, key.is_none());
    }

    #[test]
    fn test_get_asset() {
        assert_eq!(None, get_asset_amount(&PoolAsset::Ada, &mock_assets()));

        let asset =
            pool_asset_from(&String::from(CURRENCY_SYMBOL_1), &hex::encode("Tkn2")).unwrap();
        assert_eq!(Some(2), get_asset_amount(&asset, &mock_assets()));
    }
}
