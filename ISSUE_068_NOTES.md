# Issue #068: GET /wallets/:address/royalty-stats Aggregation

## Implementation Details

The royalty-stats endpoint aggregates royalty payments for artists:

- Queries `Listing` records where `originalCreator` matches the address
- Filters for `Sold` listings where `artist != originalCreator` (secondary sales)
- Aggregates royalties using: `(price * royaltyBps) / 10000`
- Returns:
  - `totalEarned`: Sum of all royalties
  - `payoutCount`: Number of secondary sales
  - `lastPayout`: Timestamp of most recent sale
- Caches with 60-second TTL via Redis

## Testing

Tests verify:
- Royalty aggregation accuracy across multiple sales
- Cache behavior (hit/miss/TTL)
- Graceful fallback when Redis is unavailable
- Correct address filtering
