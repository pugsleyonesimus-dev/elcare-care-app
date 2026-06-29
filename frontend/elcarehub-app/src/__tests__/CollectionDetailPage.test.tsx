/**
 * Tests for CollectionDetailClient — loading, loaded, and error states.
 */
import React from "react";
import { render, screen, waitFor } from "@testing-library/react";

// ── Mocks ─────────────────────────────────────────────────────────────────────

const mockMetadata = {
  name: "African Legends",
  symbol: "AFRL",
  creator: "GCREATOR",
  totalSupply: 42,
  maxSupply: 1000,
  royaltyBps: 500,
  royaltyReceiver: "GCREATOR",
};

let mockIsLoading = false;
let mockError: string | null = null;
let mockMetadataValue: typeof mockMetadata | null = null;
let mockPublicKey: string | null = null;

jest.mock("@/hooks/useLaunchpad", () => ({
  useCollectionDetail: () => ({
    metadata: mockMetadataValue,
    isLoading: mockIsLoading,
    error: mockError,
    refresh: jest.fn(),
  }),
}));

jest.mock("@/context/WalletContext", () => ({
  useWalletContext: () => ({ publicKey: mockPublicKey }),
}));

jest.mock("@/components/Navbar", () => ({
  Navbar: () => <nav data-testid="navbar" />,
}));

jest.mock("next/link", () => ({
  __esModule: true,
  default: ({ children, href }: { children: React.ReactNode; href: string }) => (
    <a href={href}>{children}</a>
  ),
}));

jest.mock("lucide-react", () =>
  Object.fromEntries(
    [
      "Loader2",
      "ShieldCheck",
      "User",
      "Percent",
      "Database",
      "Package",
      "ArrowLeft",
      "Plus",
    ].map((name) => [name, () => <span data-testid={`icon-${name}`} />])
  )
);

import CollectionDetailClient from "@/app/launchpad/collections/[address]/CollectionDetailClient";

// ── Tests ─────────────────────────────────────────────────────────────────────

describe("CollectionDetailClient", () => {
  const ADDRESS = "CCOLLECTION123";

  beforeEach(() => {
    jest.clearAllMocks();
    mockIsLoading = false;
    mockError = null;
    mockMetadataValue = null;
    mockPublicKey = null;
  });

  it("renders a loading spinner while fetching", () => {
    mockIsLoading = true;
    render(<CollectionDetailClient address={ADDRESS} />);
    expect(screen.getByTestId("icon-Loader2")).toBeInTheDocument();
  });

  it("renders an error message when loading fails", () => {
    mockError = "Network unavailable";
    render(<CollectionDetailClient address={ADDRESS} />);
    expect(screen.getByText(/error loading collection/i)).toBeInTheDocument();
    expect(screen.getByText("Network unavailable")).toBeInTheDocument();
  });

  it("renders collection name after successful load", () => {
    mockMetadataValue = mockMetadata;
    render(<CollectionDetailClient address={ADDRESS} />);
    expect(screen.getByText("African Legends")).toBeInTheDocument();
  });

  it("renders the collection symbol badge", () => {
    mockMetadataValue = mockMetadata;
    render(<CollectionDetailClient address={ADDRESS} />);
    expect(screen.getByText("AFRL")).toBeInTheDocument();
  });

  it("renders supply information", () => {
    mockMetadataValue = mockMetadata;
    render(<CollectionDetailClient address={ADDRESS} />);
    expect(screen.getByText(/42.*1000|1000.*42/)).toBeInTheDocument();
  });

  it("renders royalty percentage", () => {
    mockMetadataValue = mockMetadata;
    render(<CollectionDetailClient address={ADDRESS} />);
    expect(screen.getByText(/5\.0%/)).toBeInTheDocument();
  });

  it("renders the mint / redeem link", () => {
    mockMetadataValue = mockMetadata;
    render(<CollectionDetailClient address={ADDRESS} />);
    expect(
      screen.getByRole("link", { name: /open mint/i })
    ).toHaveAttribute("href", `/launchpad/collections/${ADDRESS}/mint`);
  });

  it("shows creator note when connected wallet is the creator", () => {
    mockMetadataValue = mockMetadata;
    mockPublicKey = "GCREATOR";
    render(<CollectionDetailClient address={ADDRESS} />);
    expect(screen.getByText(/as the creator/i)).toBeInTheDocument();
  });

  it("does not show creator note when wallet is not the creator", () => {
    mockMetadataValue = mockMetadata;
    mockPublicKey = "GBUYER";
    render(<CollectionDetailClient address={ADDRESS} />);
    expect(screen.queryByText(/as the creator/i)).not.toBeInTheDocument();
  });

  it("renders infinite symbol when maxSupply is 0", () => {
    mockMetadataValue = { ...mockMetadata, maxSupply: 0 };
    render(<CollectionDetailClient address={ADDRESS} />);
    expect(screen.getByText(/∞/)).toBeInTheDocument();
  });
});
