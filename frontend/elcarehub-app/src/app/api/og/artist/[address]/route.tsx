import { ImageResponse } from 'next/og'
import { fetchRoyaltyStats, fetchArtistListings } from '@/lib/indexer'

const CACHE_HEADERS = {
  'Cache-Control': 'public, max-age=31536000, stale-while-revalidate=86400',
}

const ERROR_CACHE_HEADERS = {
  'Cache-Control': 'public, max-age=3600, stale-while-revalidate=86400',
}

export async function GET(
  request: Request,
  { params }: { params: Promise<{ address: string }> }
) {
  const { address } = await params
  
  try {
    // Fetch artist data
    const [royaltyStats, artistListings] = await Promise.all([
      fetchRoyaltyStats(address),
      fetchArtistListings(address)
    ])

    const totalVolume = royaltyStats?.totalEarned || '0'
    const artworkCount = artistListings?.length || 0
    const totalSales = royaltyStats?.payoutCount || 0

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
              justifyContent: 'center',
              textAlign: 'center',
              padding: '60px',
              zIndex: 1,
            }}
          >
            <div
              style={{
                width: 120,
                height: 120,
                borderRadius: '50%',
                background: 'linear-gradient(135deg, #E27D60 0%, #85DCBA 100%)',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                marginBottom: 32,
                border: '4px solid rgba(255, 255, 255, 0.1)',
              }}
            >
              <span
                style={{
                  fontSize: 48,
                  fontWeight: 'bold',
                  color: 'white',
                }}
              >
                {address.slice(2, 4).toUpperCase()}
              </span>
            </div>

            <h1
              style={{
                fontSize: 54,
                fontWeight: 800,
                margin: 0,
                marginBottom: 16,
                background: 'linear-gradient(135deg, #E27D60 0%, #85DCBA 100%)',
                WebkitBackgroundClip: 'text',
                WebkitTextFillColor: 'transparent',
                backgroundClip: 'text',
              }}
            >
              African Artist Profile
            </h1>

            <p
              style={{
                fontSize: 24,
                margin: 0,
                marginBottom: 32,
                color: 'rgba(255, 255, 255, 0.7)',
                fontFamily: 'monospace',
              }}
            >
              {address.slice(0, 6)}…{address.slice(-4)}
            </p>

            <div
              style={{
                display: 'flex',
                gap: 48,
                marginBottom: 32,
              }}
            >
              <div style={{ textAlign: 'center' }}>
                <div
                  style={{
                    fontSize: 36,
                    fontWeight: 700,
                    color: '#E27D60',
                    marginBottom: 8,
                  }}
                >
                  {artworkCount}
                </div>
                <div
                  style={{
                    fontSize: 16,
                    color: 'rgba(255, 255, 255, 0.6)',
                    textTransform: 'uppercase',
                    letterSpacing: '1px',
                  }}
                >
                  Artworks
                </div>
              </div>
              
              <div style={{ textAlign: 'center' }}>
                <div
                  style={{
                    fontSize: 36,
                    fontWeight: 700,
                    color: '#85DCBA',
                    marginBottom: 8,
                  }}
                >
                  {totalSales}
                </div>
                <div
                  style={{
                    fontSize: 16,
                    color: 'rgba(255, 255, 255, 0.6)',
                    textTransform: 'uppercase',
                    letterSpacing: '1px',
                  }}
                >
                  Sales
                </div>
              </div>
              
              <div style={{ textAlign: 'center' }}>
                <div
                  style={{
                    fontSize: 36,
                    fontWeight: 700,
                    color: '#FFD700',
                    marginBottom: 8,
                  }}
                >
                  {parseFloat(totalVolume).toFixed(1)} XLM
                </div>
                <div
                  style={{
                    fontSize: 16,
                    color: 'rgba(255, 255, 255, 0.6)',
                    textTransform: 'uppercase',
                    letterSpacing: '1px',
                  }}
                >
                  Volume
                </div>
              </div>
            </div>

            <div
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: 12,
                fontSize: 18,
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
      { headers: CACHE_HEADERS }
    )
  } catch (error) {
    console.error('Failed to generate artist OG image:', error)
    
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
            <div
              style={{
                width: 120,
                height: 120,
                borderRadius: '50%',
                background: 'linear-gradient(135deg, #E27D60 0%, #85DCBA 100%)',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                marginBottom: 32,
                border: '4px solid rgba(255, 255, 255, 0.1)',
              }}
            >
              <span
                style={{
                  fontSize: 48,
                  fontWeight: 'bold',
                  color: 'white',
                }}
              >
                {address.slice(2, 4).toUpperCase()}
              </span>
            </div>
            <h1
              style={{
                fontSize: 48,
                fontWeight: 800,
                margin: 0,
                marginBottom: 16,
                textAlign: 'center',
                background: 'linear-gradient(135deg, #E27D60 0%, #85DCBA 100%)',
                WebkitBackgroundClip: 'text',
                WebkitTextFillColor: 'transparent',
                backgroundClip: 'text',
              }}
            >
              Artist data unavailable
            </h1>
            <p
              style={{
                fontSize: 20,
                margin: 0,
                color: 'rgba(255, 255, 255, 0.7)',
                fontFamily: 'monospace',
              }}
            >
              {address.slice(0, 6)}…{address.slice(-4)}
            </p>
            <div
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: 12,
                marginTop: 48,
                fontSize: 18,
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
      { headers: ERROR_CACHE_HEADERS }
    )
  }
}
