/**
 * Tests for CollectionMintClient — public mint, lazy (allowlist) redeem,
 * and sold-out / error states.
 *
 * We test CollectionMintClient directly (which accepts address as a prop)
 * to avoid the React.use(Promise) incompatibility with the test renderer.
 */
import React from "react";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

// ── Mocks ─────────────────────────────────────────────────────────────────────

const mockMint721 = jest.fn();
const mockRedeemLazy721 = jest.fn();
let mockPublicKey: string | null = "GBUYER";

const NORMAL_721_RECORD = { address: "CCOLLECTION", kind: "Normal721", creator: "GCREATOR" };
const LAZY_721_RECORD = { address: "CCOLLECTION", kind: "LazyMint721", creator: "GCREATOR" };
const DEFAULT_META = {
  name: "My Collection",
  symbol: "MC",
  creator: "GCREATOR",
  totalSupply: 5,
  maxSupply: 10,
  royaltyBps: 500,
  royaltyReceiver: "GCREATOR",
};

let mockRecord: typeof NORMAL_721_RECORD | null = NORMAL_721_RECORD;
let mockMetadataResult: typeof DEFAULT_META | null = DEFAULT_META;
let mockLoadError: string | null = null;

jest.mock("@/lib/launchpad", () => ({
  getCollectionRecordByAddress: jest.fn(async () => mockRecord),
  getCollectionMetadata: jest.fn(async () => {
    if (mockLoadError) throw new Error(mockLoadError);
    return mockMetadataResult;
  }),
  mint721: (...args: unknown[]) => mockMint721(...args),
  mint1155New: jest.fn(),
  redeemLazy721: (...args: unknown[]) => mockRedeemLazy721(...args),
  redeemLazy1155: jest.fn(),
  parseLazy721VoucherJson: jest.fn(),
  parseLazy1155VoucherJson: jest.fn(),
}));

jest.mock("@/context/WalletContext", () => ({
  useWalletContext: () => ({ publicKey: mockPublicKey }),
}));

jest.mock("@/lib/errors", () => ({
  getReadableErrorMessage: (e: unknown, fallback: string) =>
    e instanceof Error ? e.message : fallback,
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
    ["AlertCircle", "ArrowLeft", "CheckCircle2", "Loader2", "RefreshCw"].map(
      (name) => [name, () => <span data-testid={`icon-${name}`} />]
    )
  )
);

import CollectionMintClient from "@/app/launchpad/collections/[address]/mint/CollectionMintClient";

// ── Tests ─────────────────────────────────────────────────────────────────────

describe("CollectionMintClient", () => {
  const ADDRESS = "CCOLLECTION";

  beforeEach(() => {
    jest.clearAllMocks();
    mockPublicKey = "GBUYER";
    mockRecord = NORMAL_721_RECORD;
    mockMetadataResult = DEFAULT_META;
    mockLoadError = null;

    // Reset mocks on the launchpad module
    const lib = require("@/lib/launchpad");
    lib.getCollectionRecordByAddress.mockImplementation(async () => mockRecord);
    lib.getCollectionMetadata.mockImplementation(async () => {
      if (mockLoadError) throw new Error(mockLoadError);
      return mockMetadataResult;
    });
  });

  // ── Loading & Error States ─────────────────────────────────────────────────

  it("renders a loading spinner while collection loads", () => {
    const lib = require("@/lib/launchpad");
    lib.getCollectionMetadata.mockImplementation(() => new Promise(() => {}));
    render(<CollectionMintClient address={ADDRESS} />);
    expect(screen.getByTestId("icon-Loader2")).toBeInTheDocument();
  });

  it("renders an error state when collection fails to load", async () => {
    mockLoadError = "Contract not found";
    render(<CollectionMintClient address={ADDRESS} />);
    await waitFor(() =>
      expect(screen.getByText(/cannot mint/i)).toBeInTheDocument()
    );
    expect(screen.getByText(/Contract not found/)).toBeInTheDocument();
  });

  // ── Public 721 Mint ────────────────────────────────────────────────────────

  it("renders Normal721 mint form after load", async () => {
    render(<CollectionMintClient address={ADDRESS} />);
    await waitFor(() =>
      expect(screen.getByText(/mint.*721/i)).toBeInTheDocument()
    );
    expect(screen.getByPlaceholderText(/G\.\.\. destination/i)).toBeInTheDocument();
  });

  it("shows collection name in header", async () => {
    render(<CollectionMintClient address={ADDRESS} />);
    await waitFor(() =>
      expect(screen.getByText("My Collection")).toBeInTheDocument()
    );
  });

  it("calls mint721 when Mint NFT is submitted with valid data", async () => {
    // Normal 721 mint requires the creator wallet
    mockPublicKey = "GCREATOR";
    mockMint721.mockResolvedValueOnce(1);
    const user = userEvent.setup();
    render(<CollectionMintClient address={ADDRESS} />);
    await waitFor(() =>
      expect(screen.getByPlaceholderText(/G\.\.\. destination/i)).toBeInTheDocument()
    );
    const recipientInput = screen.getByPlaceholderText(/G\.\.\. destination/i);
    await user.clear(recipientInput);
    await user.type(
      recipientInput,
      "GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWN"
    );
    await user.type(
      screen.getByPlaceholderText(/ipfs:\/\//i),
      "ipfs://QmTest"
    );
    await user.click(screen.getByRole("button", { name: /mint nft/i }));
    await waitFor(() => expect(mockMint721).toHaveBeenCalledTimes(1));
  });

  it("shows success status after minting", async () => {
    // Normal 721 mint requires the creator wallet
    mockPublicKey = "GCREATOR";
    mockMint721.mockResolvedValueOnce(5);
    const user = userEvent.setup();
    render(<CollectionMintClient address={ADDRESS} />);
    await waitFor(() =>
      expect(screen.getByPlaceholderText(/G\.\.\. destination/i)).toBeInTheDocument()
    );
    const recipientInput = screen.getByPlaceholderText(/G\.\.\. destination/i);
    await user.clear(recipientInput);
    await user.type(
      recipientInput,
      "GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWN"
    );
    await user.type(
      screen.getByPlaceholderText(/ipfs:\/\//i),
      "ipfs://QmTest"
    );
    await user.click(screen.getByRole("button", { name: /mint nft/i }));
    await waitFor(() =>
      expect(screen.getByText(/submitted successfully/i)).toBeInTheDocument()
    );
  });

  // ── Sold-Out / Unregistered ────────────────────────────────────────────────

  it("shows error when collection record is null (unregistered)", async () => {
    const lib = require("@/lib/launchpad");
    lib.getCollectionRecordByAddress.mockResolvedValueOnce(null);
    render(<CollectionMintClient address={ADDRESS} />);
    await waitFor(() =>
      expect(screen.getByText(/not registered/i)).toBeInTheDocument()
    );
    expect(screen.queryByRole("button", { name: /mint nft/i })).not.toBeInTheDocument();
  });

  // ── Lazy 721 Redeem (Allowlist-style) ─────────────────────────────────────

  it("renders the lazy 721 voucher builder for LazyMint721 collections", async () => {
    const lib = require("@/lib/launchpad");
    lib.getCollectionRecordByAddress.mockResolvedValueOnce(LAZY_721_RECORD);
    render(<CollectionMintClient address={ADDRESS} />);
    await waitFor(() =>
      expect(screen.getByText(/guided voucher builder.*lazy 721/i)).toBeInTheDocument()
    );
  });

  it("Redeem & mint button is disabled when signature is empty", async () => {
    const lib = require("@/lib/launchpad");
    lib.getCollectionRecordByAddress.mockResolvedValueOnce(LAZY_721_RECORD);
    render(<CollectionMintClient address={ADDRESS} />);
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: /redeem.*mint/i })
      ).toBeDisabled()
    );
  });

  it("shows error when signature is invalid hex", async () => {
    const lib = require("@/lib/launchpad");
    lib.getCollectionRecordByAddress.mockResolvedValueOnce(LAZY_721_RECORD);
    const user = userEvent.setup();
    render(<CollectionMintClient address={ADDRESS} />);
    await waitFor(() =>
      expect(screen.getByPlaceholderText(/128 hex chars/i)).toBeInTheDocument()
    );
    await user.type(screen.getByPlaceholderText(/128 hex chars/i), "notvalidhex");
    expect(screen.getByText(/must be exactly 128 hex/i)).toBeInTheDocument();
  });

  // ── Wallet disconnected error ──────────────────────────────────────────────

  it("shows an error when wallet is not connected and mint is attempted", async () => {
    mockPublicKey = null;
    render(<CollectionMintClient address={ADDRESS} />);
    await waitFor(() =>
      expect(screen.getByPlaceholderText(/G\.\.\. destination/i)).toBeInTheDocument()
    );
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: /mint nft/i }));
    await waitFor(() =>
      expect(screen.getByText(/connect your wallet/i)).toBeInTheDocument()
    );
  });
});
