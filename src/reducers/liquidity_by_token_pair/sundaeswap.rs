use pallas::ledger::primitives::babbage::PlutusData;

use super::model::{PoolAsset, TokenPair};

#[derive(Debug, PartialEq)]
pub struct SundaePoolDatum {
    pub a: PoolAsset,
    pub b: PoolAsset,
    pub fee: f64,
    pub pool_id: String,
}

impl TryFrom<&PlutusData> for SundaePoolDatum {
    type Error = ();

    fn try_from(value: &PlutusData) -> Result<Self, Self::Error> {
        if let PlutusData::Constr(pd) = value {
            let token_pair_pd = pd.fields.get(0).ok_or(())?;
            let token_pair = TokenPair::try_from(token_pair_pd)?;

            if let (
                Some(PlutusData::BoundedBytes(pool_id_bytes)),
                Some(PlutusData::Constr(fee_pd)),
            ) = (pd.fields.get(1), pd.fields.get(3))
            {
                return match (fee_pd.fields.get(0), fee_pd.fields.get(1)) {
                    (
                        Some(PlutusData::BigInt(pallas::ledger::primitives::babbage::BigInt::Int(
                            numerator,
                        ))),
                        Some(PlutusData::BigInt(pallas::ledger::primitives::babbage::BigInt::Int(
                            denominator,
                        ))),
                    ) => {
                        let n = i32::try_from(i128::from(*numerator)).ok().ok_or(())?;
                        let d = i32::try_from(i128::from(*denominator)).ok().ok_or(())?;
                        Ok(Self {
                            a: token_pair.a,
                            b: token_pair.b,
                            fee: (n as f64) / (d as f64),
                            pool_id: hex::encode(pool_id_bytes.clone().to_vec()),
                        })
                    }
                    _ => Err(()),
                };
            }
        }

        Err(())
    }
}

impl std::fmt::Display for SundaePoolDatum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "SundaeTokenPair {{ a: ({:?}), b: ({:?}), fee: {:?} }}",
            self.a, self.b, self.fee
        )
    }
}

#[cfg(test)]
mod test {
    use pallas::ledger::primitives::{babbage::PlutusData, Fragment};

    use crate::reducers::liquidity_by_token_pair::{
        model::PoolAsset, sundaeswap::SundaePoolDatum, utils::pool_asset_from,
    };

    #[test]
    fn test_decoding_pool_datum_ada_sun() {
        let hex_pool_datum = "d8799fd8799fd8799f4040ffd8799f581c9a9693a9a37912a5097918f97918d15240c92ab729a0b7c4aa144d774653554e444145ffff41081b0000105a99e0fa59d8799f031903e8ffff";
        let data = hex::decode(hex_pool_datum).unwrap();
        let plutus_data = PlutusData::decode_fragment(&data).unwrap();
        let pool_datum = SundaePoolDatum::try_from(&plutus_data).unwrap();
        assert_eq!(PoolAsset::Ada, pool_datum.a);

        let sundae_token = pool_asset_from(
            &String::from("9a9693a9a37912a5097918f97918d15240c92ab729a0b7c4aa144d77"),
            &String::from("53554e444145"),
        )
        .unwrap();
        assert_eq!(sundae_token, pool_datum.b);
        assert_eq!(f64::from(0.003), pool_datum.fee);
    }
}
