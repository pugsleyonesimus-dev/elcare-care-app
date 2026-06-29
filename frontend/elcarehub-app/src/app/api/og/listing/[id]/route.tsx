import { ImageResponse } from 'next/og'
import { getListing, getAuction, stroopsToXlm } from '@/lib/contract'
import { fetchMetadata, cidToGatewayUrl } from '@/lib/ipfs'

const CACHE_HEADERS = {
  'Cache-Control': 'public, max-age=31536000, stale-while-revalidate=86400',
}

const NOT_FOUND_CACHE_HEADERS = {
  'Cache-Control': 'public, max-age=3600, stale-while-revalidate=86400',
}

export async function GET(
  request: Request,
  { params }: { params: Promise<{ id: string }> }
) {
  const { id } = await params
  const listingId = parseInt(id)
  
  if (isNaN(listingId)) {
    return new Response('Invalid listing ID', { status: 400 })
  }

  try {
    // Try to fetch listing first
    let listing = null
    let auction = null
    let metadata = null

    try {
      listing = await getListing(listingId)
    } catch (e) {
      // Try auction if listing fails
      try {
        auction = await getAuction(listingId)
      } catch (e) {
        // Neither found
      }
    }

    if (!listing && !auction) {
      return new ImageResponse(
        (
          <div
            style={{
              display: 'flex',
              flexDirection: 'column',
              justifyContent: 'center',
              alignItems: 'center',
              width: '100%',
              height: '100%',
              background: 'linear-gradient(135deg, #1E1E24 0%, #2D1B69 100%)',
              color: 'white',
              fontFamily: 'Inter, system-ui, sans-serif',
              position: 'relative',
            }}
          >
            <div
              style={{
                position: 'absolute',
                top: 0,
                left: 0,
                right: 0,
                bottom: 0,
                backgroundImage: `url("data:image/svg+xml,%3Csvg width='60' height='60' viewBox='0 0 60 60' xmlns='http://www.w3.org/2000/svg'%3E%3Cg fill='none' fill-rule='evenodd'%3E%3Cg fill='%23E27D60' fill-opacity='0.05'%3E%3Cpath d='M36 34v-4h-2v4h-4v2h4v4h2v-4h4v-2h-4zm0-30V0h-2v4h-4v2h4v4h2V6h4V4h-4zM6 34v-4H4v4H0v2h4v4h2v-4h4v-2H6zM6 4V0H4v4H0v2h4v4h2V6h4V4H6z'/%3E%3C/g%3E%3C/g%3E%3C/svg%3E")`,
                opacity: 0.1,
              }}
            />
            <div
              style={{
                display: 'flex',
                flexDirection: 'column',
                alignItems: 'center',
                zIndex: 1,
                padding: '60px',
              }}
            >
              <div style={{ fontSize: '72px', marginBottom: '24px' }}>🎨</div>
              <h1
                style={{
                  fontSize: '48px',
                  fontWeight: 800,
                  margin: 0,
                  marginBottom: '16px',
                  textAlign: 'center',
                  background: 'linear-gradient(135deg, #E27D60 0%, #85DCBA 100%)',
                  WebkitBackgroundClip: 'text',
                  WebkitTextFillColor: 'transparent',
                  backgroundClip: 'text',
                }}
              >
                Artwork Not Found
              </h1>
              <p
                style={{
                  fontSize: '20px',
                  margin: 0,
                  color: 'rgba(255, 255, 255, 0.7)',
                  textAlign: 'center',
                }}
              >
                Listing #{id} could not be found on ElcareHub
              </p>
              <div
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: '12px',
                  marginTop: '48px',
                  fontSize: '18px',
                  color: 'rgba(255, 255, 255, 0.5)',
                }}
              >
                <span>🎨</span>
                <span>Elcare-Hub</span>
                <span>•</span>
                <span>African Art on Stellar</span>
              </div>
            </div>
          </div>
        ),
        { headers: NOT_FOUND_CACHE_HEADERS }
      )
    }

    // Fetch metadata
    const cid = listing?.metadata_cid || auction?.metadata_cid
    if (cid) {
      metadata = await fetchMetadata(cid)
    }

    const artist = listing?.artist || auction?.creator
    const price = listing ? stroopsToXlm(listing.price) : auction ? stroopsToXlm(auction.highest_bid || auction.reserve_price) : '0'
    const status = listing?.status || auction?.status
    const imageUrl = metadata?.image ? cidToGatewayUrl(metadata.image) : null

    const title = metadata?.title || `Artwork #${id}`
    const description = metadata?.description || 'Unique African digital artwork'
    const category = metadata?.category || 'Digital Art'

    return new ImageResponse(
      (
        <div
          style={{
            display: 'flex',
            flexDirection: 'column',
            justifyContent: 'center',
            alignItems: 'center',
            width: '100%',
            height: '100%',
            background: 'linear-gradient(135deg, #1E1E24 0%, #2D1B69 100%)',
            color: 'white',
            fontFamily: 'Inter, system-ui, sans-serif',
            position: 'relative',
          }}
        >
          <div
            style={{
              position: 'absolute',
              top: 0,
              left: 0,
              right: 0,
              bottom: 0,
              backgroundImage: `url("data:image/svg+xml,%3Csvg width='60' height='60' viewBox='0 0 60 60' xmlns='http://www.w3.org/2000/svg'%3E%3Cg fill='none' fill-rule='evenodd'%3E%3Cg fill='%23E27D60' fill-opacity='0.05'%3E%3Cpath d='M36 34v-4h-2v4h-4v2h4v4h2v-4h4v-2h-4zm0-30V0h-2v4h-4v2h4v4h2V6h4V4h-4zM6 34v-4H4v4H0v2h4v4h2v-4h4v-2H6zM6 4V0H4v4H0v2h4v4h2V6h4V4H6z'/%3E%3C/g%3E%3C/g%3E%3C/svg%3E")`,
              opacity: 0.1,
            }}
          />
          
          <div
            style={{
              display: 'flex',
              width: '100%',
              height: '100%',
              position: 'relative',
              zIndex: 1,
            }}
          >
            <div
              style={{
                flex: 1,
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                padding: '40px',
                position: 'relative',
              }}
            >
              {imageUrl ? (
                <img
                  src={imageUrl}
                  alt={title}
                  style={{
                    width: '100%',
                    height: '100%',
                    objectFit: 'cover',
                    borderRadius: '20px',
                    boxShadow: '0 20px 40px rgba(0, 0, 0, 0.3)',
                  }}
                />
              ) : (
                <div
                  style={{
                    width: '100%',
                    height: '100%',
                    background: 'linear-gradient(135deg, #E27D60 0%, #85DCBA 100%)',
                    borderRadius: '20px',
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    fontSize: '72px',
                    boxShadow: '0 20px 40px rgba(0, 0, 0, 0.3)',
                  }}
                >
                  🎨
                </div>
              )}
              
              <div
                style={{
                  position: 'absolute',
                  top: '60px',
                  left: '60px',
                  background: status === 'Active' ? '#10B981' : status === 'Sold' || status === 'Finalized' ? '#E27D60' : '#EF4444',
                  color: 'white',
                  padding: '8px 16px',
                  borderRadius: '20px',
                  fontSize: '14px',
                  fontWeight: 'bold',
                  textTransform: 'uppercase',
                  letterSpacing: '1px',
                }}
              >
                {status}
              </div>
            </div>

            <div
              style={{
                flex: 1,
                display: 'flex',
                flexDirection: 'column',
                justifyContent: 'center',
                padding: '60px 60px 60px 20px',
              }}
            >
              <h1
                style={{
                  fontSize: '48px',
                  fontWeight: 800,
                  margin: 0,
                  marginBottom: '16px',
                  lineHeight: 1.2,
                  background: 'linear-gradient(135deg, #E27D60 0%, #85DCBA 100%)',
                  WebkitBackgroundClip: 'text',
                  WebkitTextFillColor: 'transparent',
                  backgroundClip: 'text',
                }}
              >
                {title}
              </h1>

              <p
                style={{
                  fontSize: '20px',
                  margin: 0,
                  marginBottom: '12px',
                  color: 'rgba(255, 255, 255, 0.8)',
                  fontFamily: 'monospace',
                }}
              >
                by {artist?.slice(0, 6)}…{artist?.slice(-4)}
              </p>

              <p
                style={{
                  fontSize: '16px',
                  margin: 0,
                  marginBottom: '24px',
                  color: 'rgba(255, 255, 255, 0.6)',
                  textTransform: 'uppercase',
                  letterSpacing: '1px',
                }}
              >
                {category}
              </p>

              <div
                style={{
                  display: 'flex',
                  alignItems: 'baseline',
                  gap: '8px',
                  marginBottom: '24px',
                }}
              >
                <span
                  style={{
                    fontSize: '36px',
                    fontWeight: 700,
                    color: '#FFD700',
                  }}
                >
                  {price}
                </span>
                <span
                  style={{
                    fontSize: '18px',
                    color: 'rgba(255, 255, 255, 0.8)',
                    fontWeight: 500,
                  }}
                >
                  XLM
                </span>
              </div>

              <div
                style={{
                  display: 'inline-block',
                  background: 'rgba(255, 255, 255, 0.1)',
                  color: 'white',
                  padding: '8px 16px',
                  borderRadius: '12px',
                  fontSize: '14px',
                  fontWeight: 'bold',
                  marginBottom: '24px',
                  backdropFilter: 'blur(10px)',
                  border: '1px solid rgba(255, 255, 255, 0.2)',
                }}
              >
                {listing ? '🏪 Fixed Price' : '🎵 Timed Auction'}
              </div>

              <div
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: '12px',
                  fontSize: '16px',
                  color: 'rgba(255, 255, 255, 0.5)',
                  marginTop: 'auto',
                }}
              >
                <span>🎨</span>
                <span>Elcare-Hub</span>
                <span>•</span>
                <span>African Art on Stellar</span>
              </div>
            </div>
          </div>
        </div>
      ),
      { headers: CACHE_HEADERS }
    )
  } catch (error) {
    console.error('Failed to generate listing OG image:', error)
    
    return new ImageResponse(
      (
        <div
          style={{
            display: 'flex',
            flexDirection: 'column',
            justifyContent: 'center',
            alignItems: 'center',
            width: '100%',
            height: '100%',
            background: 'linear-gradient(135deg, #1E1E24 0%, #2D1B69 100%)',
            color: 'white',
            fontFamily: 'Inter, system-ui, sans-serif',
            position: 'relative',
          }}
        >
          <div
            style={{
              position: 'absolute',
              top: 0,
              left: 0,
              right: 0,
              bottom: 0,
              backgroundImage: `url("data:image/svg+xml,%3Csvg width='60' height='60' viewBox='0 0 60 60' xmlns='http://www.w3.org/2000/svg'%3E%3Cg fill='none' fill-rule='evenodd'%3E%3Cg fill='%23E27D60' fill-opacity='0.05'%3E%3Cpath d='M36 34v-4h-2v4h-4v2h4v4h2v-4h4v-2h-4zm0-30V0h-2v4h-4v2h4v4h2V6h4V4h-4zM6 34v-4H4v4H0v2h4v4h2v-4h4v-2H6zM6 4V0H4v4H0v2h4v4h2V6h4V4H6z'/%3E%3C/g%3E%3C/g%3E%3C/svg%3E")`,
              opacity: 0.1,
            }}
          />
          <div
            style={{
              display: 'flex',
              flexDirection: 'column',
              alignItems: 'center',
              zIndex: 1,
              padding: '60px',
            }}
          >
            <div style={{ fontSize: '72px', marginBottom: '24px' }}>🎨</div>
            <h1
              style={{
                fontSize: '48px',
                fontWeight: 800,
                margin: 0,
                marginBottom: '16px',
                textAlign: 'center',
                background: 'linear-gradient(135deg, #E27D60 0%, #85DCBA 100%)',
                WebkitBackgroundClip: 'text',
                WebkitTextFillColor: 'transparent',
                backgroundClip: 'text',
              }}
            >
              Artwork #{id}
            </h1>
            <p
              style={{
                fontSize: '20px',
                margin: 0,
                color: 'rgba(255, 255, 255, 0.7)',
                textAlign: 'center',
              }}
            >
              Elcare-Hub - African Art on Stellar
            </p>
          </div>
        </div>
      ),
      { headers: NOT_FOUND_CACHE_HEADERS }
    )
  }
}
