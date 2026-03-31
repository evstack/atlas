-- Recompute ERC-20 total supply from indexed balances.
-- Metadata fetches only capture a point-in-time value, which goes stale for
-- mintable or burnable tokens during historical indexing.
UPDATE erc20_contracts AS c
SET total_supply = COALESCE(b.total_supply, 0)
FROM (
    SELECT
        erc20_contracts.address,
        COALESCE(SUM(balance), 0) AS total_supply
    FROM erc20_contracts
    LEFT JOIN erc20_balances ON erc20_balances.contract_address = erc20_contracts.address
        AND erc20_balances.balance > 0
    GROUP BY erc20_contracts.address
) AS b
WHERE c.address = b.address;
