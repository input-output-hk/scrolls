# Liquidity by Token Pair Reducer

## Introduction

This reducer intends to aggregate changes across different AMM DEXs (decentralized exchanges). It currently supports the most popular ones which includes:

- MinSwap
- Muesliswap
- SundaeSwap
- Wingriders

### Note

> Muesliswap is considered a hybrid DEX, offering orderbook liquidity and liquidity via pool in form of an AMM DEX. This reducer currently only observes its liquidity pools.

## Configuration

- `pool_currency_symbol` required hex-encoded currency symbol of the token that marks valid liquidity pool unspent transaction outputs (UTxOs)
- `pool_prefix` optional prefix for Redis key
- `dex_prefix` optional prefix for Redis members (usually used to prefix different liquidity sources by unique dex prefix)

## How it works

The reducer was implemented to be used for Redis. Hence, it produces key/value pairs for different liquidity sources. Thereby, a redis key represents a token pair (`a`, `b`) for which one or more liquidity pools exist from different DEXs. `a` and `b` are defined as `PoolAsset` which is an enum with two variants: `Ada` or `NativeAsset(currency_symbol, token_name)`.

### Redis Key Schema

A Redis key for a token pair for which at least one liquidity pool exists follows the following schema:

(`<pool_prefix>.`)?(`<currency_symbol_a>.<a_token_name>:` | `<empty>`)(`<currency_symbol_b>.<b_token_name>`)

Any key may have a prefix that can be optionally defined via the `pool_prefix` configuration (see section below). A single token is identified by its `currency_symbol` plus `token_name` encoded in `hex`.
Any two tokens that make up a Redis key are sorted alphanumerically so that the smaller key comes first. This is required to have consistency across all liquidity pairs from different DEXs and also affects the member for a Redis key which hold liquidity ratios of each token. But more on that later.
Liquidity pools that provide liquidity for `ADA` and some native asset always result in a Redis key that has the following format:

(`<pool_prefix>.`)?(`<currency_symbol_b>.<b_token_name>`)

Since ADA has an empty currency symbol and token name a corresponding Redis key look as follows.

Example ADA/WRT liquidity key with `pool` prefix:
https://preprod.cardanoscan.io/token/659ab0b5658687c2e74cd10dba8244015b713bf503b90557769d77a757696e67526964657273

`pool.659ab0b5658687c2e74cd10dba8244015b713bf503b90557769d77a7.57696e67526964657273`

### Redis Value Schema

The reducer's value is a set. Each entry is a single liquidity source that is json encoded. A single member can contain up to five fields:

- dex specific prefix to identify the origin of the liquidity source
- amount of token a
- amount of token b
- a decimal number defining the fee of the liquidity source that's paid to liquidity providers _(optional)_
- a pool_id encoded base16 \*(optional)\_ only available for Sundaeswap liquidity pools

Below you can find the general schema for a JSON encoded member:

```
{
  "token_a": string,
  "token_b": string,
  "dex": string,
  "fee": number,
  "pool_id": string
}
```

Example ADA/MIN liquidity source from SundaeSwap DEX:

```
{
  "token_a": "31249392392",
  "token_b": "1323123231221",
  "dex": "sun",
  "fee": 0.003,
  "pool_id": "2d01"
}
```

### How to get the right price from `token_a` amount and `token_b` amount?

Given the information of some liquidity pool described above, one can now divide both amounts for `token_a` and `token_b` to get a **non-normalized**
price. It's **non-normalized** because each native asseton Cardano defines its own number of decimal places. Hence, this ratio is likely off by a few decimals.
In order to get the correct price, one needs to look up the number of decimals using the token registry - or in case of `ADA` use it is 10^6. In other words: 1 ADA = 1.000.000 lovelace.

Therefore, in general given a token pair `a`, `b` including respective amounts and decimal places for each token, the price can be derived as follows:

```
price_a_b = amount(a) / amount(b) * 10^(decimal_place(a) - decimal_place(b))
```

### How to run

An example can be found in the [testdrive](https://github.com/input-output-hk/scrolls/tree/liquidity_by_token_pair/testdrive/liquidity_by_token_pair) directory for preprod and mainnet. It also comes with a `docker-compose` file. Currently, as this forked repository does not have any official releases, the `docker-compose`references a locally built image that can be produced by running the following command in the root directory of this repository:

```
docker build -t lace/scrolls:staging .
```

Note: For Macs with ARM, you need to enable docker to use Rosetta x86/amd64 emulation. If it still doesn't work, `export DOCKER_DEFAULT_PLATFORM=linux/amd64` before building might help.

After that, a new docker image was produced which contains the `scrolls` binary with the new `LiquidityByTokenPair` reducer.

Next, we can change directories back to the `testdrive/liquidity_by_token_pair/<network>` and choose one of the two provided examples for different networks (`mainnet` or `preprod`).
Within the network directory of your choice run:

```
docker-compose up -d
```

This creates two new docker containers. One for `scrolls` and another one for `redis`. If you haven't changed the `docker-compose` file, you'll see we actually deploy a `redis-stack` container and not just a plain `redis` instance.
Now, we can navigate to `localhost:8001` and use RedisInsight to see real time updates coming in from the blockchain, block by block. It can sometimes take up to 30 - 60 seconds until you see something depending on when the next block is produced that contains a change in liquidity from any of the DEXs we're observing.

Lastly, if you would like to see real time logging from the containers, you can run:

```
docker-compose logs -f
```

You should see redis requesting next blocks and updating its cursor which represents the current point of the chain it looks at (pair of slot number and block hash).
