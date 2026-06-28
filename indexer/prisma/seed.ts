import { PrismaClient } from '@prisma/client';

const prisma = new PrismaClient();

async function main() {
  console.log('🌱 Seeding database with representative data...');

  // Clear existing data (idempotent)
  await prisma.$transaction([
    prisma.bid.deleteMany(),
    prisma.offer.deleteMany(),
    prisma.auction.deleteMany(),
    prisma.listing.deleteMany(),
    prisma.collection.deleteMany(),
    prisma.marketplaceEvent.deleteMany(),
  ]);

  // Create collections
  const collection1 = await prisma.collection.create({
    data: {
      contractAddress: 'CAQBWUKVLOR5W43QBQDFJAHSE2LUGCALRDCM7EVEO36FTWOP5P2O36ML',
      kind: 'normal_1155',
      creator: 'GBFUNHEQOVN35LFEKP7SZXFYJPMJ3WLXLX4PQZGBK737NTLRHOKVES3F',
      name: 'African Heritage NFTs',
      symbol: 'AHT',
      deployedAtLedger: 1000,
    },
  });

  const collection2 = await prisma.collection.create({
    data: {
      contractAddress: 'CA4RKSR4ORRIFBBW64MXCWS7GGJ4GY6AIXRGU5EGS43XBDDB7OYV3TRG',
      kind: 'normal_721',
      creator: 'GBD4U46RSIVHVGNGHSRUQQ7SVU7ISA26FTJSTPVYS4IRAHYRNECKUIGHTS',
      name: 'Tingatinga Collection',
      symbol: 'TTG',
      deployedAtLedger: 1050,
    },
  });

  // Create listings
  const listing1 = await prisma.listing.create({
    data: {
      listingId: 101n,
      artist: 'GBFUNHEQOVN35LFEKP7SZXFYJPMJ3WLXLX4PQZGBK737NTLRHOKVES3F',
      owner: null,
      price: '100.0000000',
      currency: 'XLM',
      collection: collection1.contractAddress,
      nftTokenId: 1n,
      token: 'native',
      status: 'Active',
      recipients: [{ address: 'GBFUNHEQOVN35LFEKP7SZXFYJPMJ3WLXLX4PQZGBK737NTLRHOKVES3F', percentage: 100 }],
      createdAtLedger: 1100,
      updatedAtLedger: 1100,
    },
  });

  const listing2 = await prisma.listing.create({
    data: {
      listingId: 102n,
      artist: 'GBD4U46RSIVHVGNGHSRUQQ7SVU7ISA26FTJSTPVYS4IRAHYRNECKUIGHTS',
      owner: 'GBUYC3APGYBFXAQU3DXJQFZJKFLVH4K3BQZ3TQHUWJ3K3TQHUWJ3K3BQZ3',
      price: '250.0000000',
      currency: 'XLM',
      collection: collection2.contractAddress,
      nftTokenId: 2n,
      token: 'native',
      status: 'Sold',
      recipients: [{ address: 'GBD4U46RSIVHVGNGHSRUQQ7SVU7ISA26FTJSTPVYS4IRAHYRNECKUIGHTS', percentage: 100 }],
      createdAtLedger: 1150,
      updatedAtLedger: 1200,
    },
  });

  // Create an auction
  const auction1 = await prisma.auction.create({
    data: {
      auctionId: 201n,
      creator: 'GBFUNHEQOVN35LFEKP7SZXFYJPMJ3WLXLX4PQZGBK737NTLRHOKVES3F',
      collection: collection1.contractAddress,
      nftTokenId: 3n,
      token: 'native',
      reservePrice: '50.0000000',
      highestBid: '150.0000000',
      highestBidder: 'GBD4U46RSIVHVGNGHSRUQQ7SVU7ISA26FTJSTPVYS4IRAHYRNECKUIGHTS',
      endTime: 2000000000n,
      status: 'Active',
      recipients: [{ address: 'GBFUNHEQOVN35LFEKP7SZXFYJPMJ3WLXLX4PQZGBK737NTLRHOKVES3F', percentage: 100 }],
      createdAtLedger: 1250,
      updatedAtLedger: 1300,
    },
  });

  // Create bids for the auction
  await prisma.bid.create({
    data: {
      auctionId: auction1.auctionId,
      bidder: 'GCXVVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVX',
      amount: '75.0000000',
      ledgerSequence: 1310,
    },
  });

  await prisma.bid.create({
    data: {
      auctionId: auction1.auctionId,
      bidder: 'GBD4U46RSIVHVGNGHSRUQQ7SVU7ISA26FTJSTPVYS4IRAHYRNECKUIGHTS',
      amount: '150.0000000',
      ledgerSequence: 1320,
    },
  });

  // Create offers
  const offer1 = await prisma.offer.create({
    data: {
      offerId: 301n,
      listingId: listing1.listingId,
      offerer: 'GCXVVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVX',
      amount: '120.0000000',
      token: 'native',
      status: 'Pending',
      createdAtLedger: 1350,
      updatedAtLedger: 1350,
    },
  });

  // Create marketplace events
  await prisma.marketplaceEvent.create({
    data: {
      listingId: listing1.listingId,
      eventType: 'LISTING_CREATED',
      actor: listing1.artist,
      data: {
        listing_id: listing1.listingId.toString(),
        artist: listing1.artist,
        price: listing1.price,
        currency: listing1.currency,
      },
      ledgerSequence: 1100,
    },
  });

  await prisma.marketplaceEvent.create({
    data: {
      listingId: listing2.listingId,
      eventType: 'ARTWORK_SOLD',
      actor: 'GBUYC3APGYBFXAQU3DXJQFZJKFLVH4K3BQZ3TQHUWJ3K3TQHUWJ3K3BQZ3',
      data: {
        listing_id: listing2.listingId.toString(),
        buyer: listing2.owner,
        price: listing2.price,
      },
      ledgerSequence: 1200,
    },
  });

  await prisma.marketplaceEvent.create({
    data: {
      listingId: auction1.auctionId,
      eventType: 'AUCTION_CREATED',
      actor: auction1.creator,
      data: {
        auction_id: auction1.auctionId.toString(),
        creator: auction1.creator,
        reserve_price: auction1.reservePrice,
        end_time: auction1.endTime.toString(),
      },
      ledgerSequence: 1250,
    },
  });

  console.log('✅ Seed completed successfully!');
  console.log(`  - Collections: 2`);
  console.log(`  - Listings: 2`);
  console.log(`  - Auctions: 1`);
  console.log(`  - Bids: 2`);
  console.log(`  - Offers: 1`);
  console.log(`  - Events: 3`);
}

main()
  .catch((e) => {
    console.error('❌ Seeding failed:', e);
    process.exit(1);
  })
  .finally(async () => {
    await prisma.$disconnect();
  });
