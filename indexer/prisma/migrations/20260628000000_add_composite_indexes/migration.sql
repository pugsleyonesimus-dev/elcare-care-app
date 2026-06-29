-- AddIndex: Composite indexes for common listing filter+sort patterns
CREATE INDEX "Listing_status_updatedAtLedger_idx" ON "Listing"("status", "updatedAtLedger");
CREATE INDEX "Listing_artist_updatedAtLedger_idx" ON "Listing"("artist", "updatedAtLedger");
CREATE INDEX "Listing_collection_idx" ON "Listing"("collection");
CREATE INDEX "Listing_collection_status_idx" ON "Listing"("collection", "status");
