import React from 'react'
import { render, screen } from '@testing-library/react'

jest.mock('next/og', () => ({
  ImageResponse: jest.fn().mockImplementation((element, options) => ({
    element,
    options,
    status: options?.status || 200,
  })),
}))

jest.mock('@/lib/contract', () => ({
  getListing: jest.fn(),
  getAuction: jest.fn(),
  stroopsToXlm: jest.fn().mockReturnValue('1'),
}))

jest.mock('@/lib/ipfs', () => ({
  fetchMetadata: jest.fn(),
  cidToGatewayUrl: jest.fn((cid: string) => `https://gateway.example.com/ipfs/${cid.replace('ipfs://', '')}`),
}))

import { GET } from '@/app/api/og/listing/[id]/route'
import { getListing, getAuction } from '@/lib/contract'
import { fetchMetadata } from '@/lib/ipfs'

const mockGetListing = getListing as jest.Mock
const mockGetAuction = getAuction as jest.Mock
const mockFetchMetadata = fetchMetadata as jest.Mock

function callGet(id: string) {
  return GET(
    new Request(`http://localhost:3000/api/og/listing/${id}`),
    { params: Promise.resolve({ id }) }
  )
}

beforeEach(() => {
  jest.clearAllMocks()
})

describe('OG Listing Route — invalid input', () => {
  it('returns 400 for non-numeric id', async () => {
    const res = await callGet('abc')
    expect(res.status).toBe(400)
    expect(await res.text()).toBe('Invalid listing ID')
  })

  it('returns 400 for NaN id', async () => {
    const res = await callGet('NaN')
    expect(res.status).toBe(400)
    expect(await res.text()).toBe('Invalid listing ID')
  })
})

describe('OG Listing Route — valid listing data', () => {
  it('renders with listing data and sets long cache headers', async () => {
    mockGetListing.mockResolvedValue({
      listing_id: 1,
      artist: 'GABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890',
      metadata_cid: 'QmTest123',
      price: 10000000n,
      status: 'Active',
    })
    mockGetAuction.mockRejectedValue(new Error('Not found'))
    mockFetchMetadata.mockResolvedValue({
      title: 'Test Artwork',
      description: 'A test artwork',
      artist: 'Test Artist',
      image: 'ipfs://QmImage123',
      year: '2024',
      category: 'Digital Art',
    })

    const res = await callGet('1')
    expect(res.status).toBe(200)
    expect(res.options?.headers?.['Cache-Control']).toBe(
      'public, max-age=31536000, stale-while-revalidate=86400'
    )

    render(res.element)
    expect(screen.getByText('Test Artwork')).toBeInTheDocument()
    expect(screen.getByText(/GABCDE…7890/)).toBeInTheDocument()
    expect(screen.getByText('Digital Art')).toBeInTheDocument()
    expect(screen.getByText('1')).toBeInTheDocument()
    expect(screen.getByText('XLM')).toBeInTheDocument()
    expect(screen.getByText('🏪 Fixed Price')).toBeInTheDocument()
  })

  it('renders with auction data when listing throws', async () => {
    mockGetListing.mockRejectedValue(new Error('Not found'))
    mockGetAuction.mockResolvedValue({
      auction_id: 1,
      creator: 'GZYXWVUTSRQPONMLKJIHGFEDCBA9876543210',
      metadata_cid: 'QmAuction999',
      reserve_price: 5000000n,
      highest_bid: 5000000n,
      status: 'Active',
    })
    mockFetchMetadata.mockResolvedValue({
      title: 'Auction Piece',
      description: '',
      artist: '',
      image: '',
      year: '2024',
      category: 'Sculpture',
    })

    const res = await callGet('1')
    expect(res.status).toBe(200)
    expect(res.options?.headers?.['Cache-Control']).toContain('public')

    render(res.element)
    expect(screen.getByText('Auction Piece')).toBeInTheDocument()
    expect(screen.getByText('🎵 Timed Auction')).toBeInTheDocument()
  })
})

describe('OG Listing Route — missing data fallback', () => {
  it('renders branded "Artwork Not Found" when both listing and auction are missing', async () => {
    mockGetListing.mockRejectedValue(new Error('Not found'))
    mockGetAuction.mockRejectedValue(new Error('Not found'))

    const res = await callGet('999')
    expect(res.status).toBe(200)
    expect(res.options?.headers?.['Cache-Control']).toContain('max-age=3600')

    render(res.element)
    expect(screen.getByText('Artwork Not Found')).toBeInTheDocument()
    expect(screen.getByText(/Listing #999 could not be found on ElcareHub/)).toBeInTheDocument()
  })

  it('renders branded fallback with id when metadata fetch fails', async () => {
    mockGetListing.mockResolvedValue({
      listing_id: 1,
      artist: 'GABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890',
      metadata_cid: 'QmBroken',
      price: 10000000n,
      status: 'Active',
    })
    mockGetAuction.mockRejectedValue(new Error('Not found'))
    mockFetchMetadata.mockRejectedValue(new Error('IPFS error'))

    const res = await callGet('1')
    expect(res.status).toBe(200)
    expect(res.options?.headers?.['Cache-Control']).toContain('max-age=3600')

    render(res.element)
    expect(screen.getByText('Artwork #1')).toBeInTheDocument()
    expect(screen.getByText('Elcare-Hub - African Art on Stellar')).toBeInTheDocument()
  })
})
