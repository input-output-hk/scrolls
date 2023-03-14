use pallas::ledger::primitives::babbage::PlutusData;

use super::model::{PoolAsset, TokenPair};

pub struct MuesliSwapPoolDatum {
    pub a: PoolAsset,
    pub b: PoolAsset,
}

impl TryFrom<&PlutusData> for MuesliSwapPoolDatum {
    type Error = ();

    fn try_from(value: &PlutusData) -> Result<Self, Self::Error> {
        if let Some(TokenPair { a, b }) = TokenPair::try_from(value).ok() {
            return Ok(Self { a, b });
        }

        Err(())
    }
}

#[cfg(test)]
mod test {
    use pallas::ledger::primitives::{babbage::PlutusData, Fragment};

    use crate::reducers::liquidity_by_token_pair::{
        model::PoolAsset, muesliswap::MuesliSwapPoolDatum, utils::pool_asset_from,
    };

    #[test]
    fn test_decoding_pool_datum_ada_min() {
        let hex_pool_datum = "d8799fd8799f4040ffd8799f581c29d222ce763455e3d7a09a665ce554f00ac89d2e99a1a83d267170c6434d494eff1a9041264e181eff";
        let data = hex::decode(hex_pool_datum).unwrap();
        let plutus_data = PlutusData::decode_fragment(&data).unwrap();
        let pool_datum = MuesliSwapPoolDatum::try_from(&plutus_data).unwrap();
        assert_eq!(PoolAsset::Ada, pool_datum.a);
        let minswap_token = pool_asset_from(
            &String::from("29d222ce763455e3d7a09a665ce554f00ac89d2e99a1a83d267170c6"),
            &String::from("4d494e"),
        )
        .unwrap();
        assert_eq!(minswap_token, pool_datum.b);
    }
}
