import React from 'react'
import { render, screen } from '@testing-library/react'

jest.mock('next/og', () => ({
  ImageResponse: jest.fn().mockImplementation((element, options) => ({
    element,
    options,
    status: options?.status || 200,
  })),
}))

jest.mock('@/lib/indexer', () => ({
  fetchRoyaltyStats: jest.fn(),
  fetchArtistListings: jest.fn(),
}))

import { GET } from '@/app/api/og/artist/[address]/route'
import { fetchRoyaltyStats, fetchArtistListings } from '@/lib/indexer'

const mockFetchRoyaltyStats = fetchRoyaltyStats as jest.Mock
const mockFetchArtistListings = fetchArtistListings as jest.Mock

const TEST_ADDRESS = 'GABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890'

function callGet(address: string = TEST_ADDRESS) {
  return GET(
    new Request(`http://localhost:3000/api/og/artist/${address}`),
    { params: Promise.resolve({ address }) }
  )
}

beforeEach(() => {
  jest.clearAllMocks()
})

describe('OG Artist Route — valid data', () => {
  it('renders with artist stats and sets long cache headers', async () => {
    mockFetchRoyaltyStats.mockResolvedValue({
      totalEarned: '150.5',
      payoutCount: 12,
      lastPayout: 1700000000,
    })
    mockFetchArtistListings.mockResolvedValue([{ id: 1 }, { id: 2 }, { id: 3 }])

    const res = await callGet()
    expect(res.status).toBe(200)
    expect(res.options?.headers?.['Cache-Control']).toBe(
      'public, max-age=31536000, stale-while-revalidate=86400'
    )

    render(res.element)
    expect(screen.getByText('African Artist Profile')).toBeInTheDocument()
    expect(screen.getByText(/GABCDE…7890/)).toBeInTheDocument()
    expect(screen.getByText('3')).toBeInTheDocument()
    expect(screen.getByText('12')).toBeInTheDocument()
    expect(screen.getByText(/150.5 XLM/)).toBeInTheDocument()
  })
})

describe('OG Artist Route — empty / no data', () => {
  it('renders with zero stats when artist has no activity', async () => {
    mockFetchRoyaltyStats.mockResolvedValue({
      totalEarned: '0',
      payoutCount: 0,
      lastPayout: 0,
    })
    mockFetchArtistListings.mockResolvedValue([])

    const res = await callGet()
    expect(res.status).toBe(200)
    expect(res.options?.headers?.['Cache-Control']).toContain('max-age=31536000')

    render(res.element)
    expect(screen.getByText('African Artist Profile')).toBeInTheDocument()
    expect(screen.getByText(/GABCDE…7890/)).toBeInTheDocument()
  })
})

describe('OG Artist Route — error fallback', () => {
  it('renders branded fallback when indexer throws', async () => {
    mockFetchRoyaltyStats.mockRejectedValue(new Error('Indexer unavailable'))
    mockFetchArtistListings.mockRejectedValue(new Error('Indexer unavailable'))

    const res = await callGet()
    expect(res.status).toBe(200)
    expect(res.options?.headers?.['Cache-Control']).toContain('max-age=3600')

    render(res.element)
    expect(screen.getByText('Artist data unavailable')).toBeInTheDocument()
    expect(screen.getByText(/GABCDE…7890/)).toBeInTheDocument()
  })

  it('renders branded fallback when only one endpoint throws', async () => {
    mockFetchRoyaltyStats.mockResolvedValue({
      totalEarned: '0',
      payoutCount: 0,
      lastPayout: 0,
    })
    mockFetchArtistListings.mockRejectedValue(new Error('Network error'))

    const res = await callGet()
    expect(res.status).toBe(200)
    expect(res.options?.headers?.['Cache-Control']).toContain('max-age=3600')

    render(res.element)
    expect(screen.getByText('Artist data unavailable')).toBeInTheDocument()
  })

  it('shows correct avatar initials from address', async () => {
    mockFetchRoyaltyStats.mockRejectedValue(new Error('Down'))
    mockFetchArtistListings.mockRejectedValue(new Error('Down'))

    const customAddress = 'GXYZABCDEF1234567890JUSTATESTADDR'
    const res = await callGet(customAddress)
    render(res.element)

    expect(screen.getByText('YZ')).toBeInTheDocument()
  })
})
