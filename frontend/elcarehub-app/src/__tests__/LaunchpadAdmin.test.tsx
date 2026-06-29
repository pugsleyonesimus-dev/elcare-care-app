/**
 * Tests for the Launchpad Admin page and hooks.
 */
import React from "react";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

// ── Mocks ─────────────────────────────────────────────────────────────────────

let mockPublicKey: string | null = "GADMIN";

const mockGetLaunchpadAdmin = jest.fn();
const mockGetCollectionCount = jest.fn();
const mockGetPlatformFee = jest.fn();
const mockGetAllCollections = jest.fn();
const mockTransferLaunchpadAdmin = jest.fn();
const mockUpdatePlatformFee = jest.fn();

jest.mock("@/hooks/useWallet", () => ({
  useWallet: () => ({ publicKey: mockPublicKey }),
}));

jest.mock("@/lib/launchpad", () => ({
  getLaunchpadAdmin: (...args: unknown[]) => mockGetLaunchpadAdmin(...args),
  getCollectionCount: (...args: unknown[]) => mockGetCollectionCount(...args),
  getPlatformFee: (...args: unknown[]) => mockGetPlatformFee(...args),
  getAllCollections: (...args: unknown[]) => mockGetAllCollections(...args),
  transferLaunchpadAdmin: (...args: unknown[]) =>
    mockTransferLaunchpadAdmin(...args),
  updatePlatformFee: (...args: unknown[]) => mockUpdatePlatformFee(...args),
}));

jest.mock("@/lib/config", () => ({
  config: {
    contractId: "CTEST",
    horizonUrl: "https://horizon-testnet.stellar.org",
    networkPassphrase: "Test SDF Network ; September 2015",
    network: "testnet",
    rpcUrl: "https://soroban-testnet.stellar.org",
    launchpadContractId: "CLAUNCHPAD",
    indexerUrl: "",
  },
}));

jest.mock("lucide-react", () =>
  Object.fromEntries(
    [
      "Shield",
      "Settings",
      "TrendingUp",
      "Users",
      "DollarSign",
      "Edit",
      "Save",
      "X",
      "Loader2",
      "AlertCircle",
      "CheckCircle2",
      "Palette",
      "BarChart3",
      "Crown",
      "Zap",
    ].map((name) => [name, ({ size }: { size?: number }) => <span data-testid={`icon-${name}`} data-size={size} />])
  )
);

import {
  useLaunchpadAdminCheck,
  useLaunchpadAdminStats,
  useLaunchpadAdminActions,
} from "@/hooks/useLaunchpadAdmin";

// ── useLaunchpadAdminCheck ────────────────────────────────────────────────────

describe("useLaunchpadAdminCheck", () => {
  beforeEach(() => jest.clearAllMocks());

  it("returns isAdmin=false when publicKey is null", async () => {
    function Comp() {
      const { isAdmin, isLoading } = useLaunchpadAdminCheck(null);
      return (
        <div>
          <span data-testid="admin">{String(isAdmin)}</span>
          <span data-testid="loading">{String(isLoading)}</span>
        </div>
      );
    }
    render(<Comp />);
    await waitFor(() =>
      expect(screen.getByTestId("loading").textContent).toBe("false")
    );
    expect(screen.getByTestId("admin").textContent).toBe("false");
  });

  it("returns isAdmin=true when publicKey matches contract admin", async () => {
    mockGetLaunchpadAdmin.mockResolvedValueOnce("GADMIN");
    function Comp() {
      const { isAdmin, isLoading } = useLaunchpadAdminCheck("GADMIN");
      return (
        <div>
          <span data-testid="admin">{String(isAdmin)}</span>
          <span data-testid="loading">{String(isLoading)}</span>
        </div>
      );
    }
    render(<Comp />);
    await waitFor(() =>
      expect(screen.getByTestId("loading").textContent).toBe("false")
    );
    expect(screen.getByTestId("admin").textContent).toBe("true");
  });

  it("returns isAdmin=false when publicKey does not match", async () => {
    mockGetLaunchpadAdmin.mockResolvedValueOnce("GADMIN");
    function Comp() {
      const { isAdmin } = useLaunchpadAdminCheck("GNOTADMIN");
      return <span data-testid="admin">{String(isAdmin)}</span>;
    }
    render(<Comp />);
    await waitFor(() =>
      expect(screen.getByTestId("admin").textContent).toBe("false")
    );
  });

  it("sets isAdmin=false when contract call fails", async () => {
    mockGetLaunchpadAdmin.mockRejectedValueOnce(new Error("rpc down"));
    function Comp() {
      const { isAdmin, isLoading } = useLaunchpadAdminCheck("GADMIN");
      return (
        <div>
          <span data-testid="admin">{String(isAdmin)}</span>
          <span data-testid="loading">{String(isLoading)}</span>
        </div>
      );
    }
    render(<Comp />);
    await waitFor(() =>
      expect(screen.getByTestId("loading").textContent).toBe("false")
    );
    expect(screen.getByTestId("admin").textContent).toBe("false");
  });
});

// ── useLaunchpadAdminStats ────────────────────────────────────────────────────

describe("useLaunchpadAdminStats", () => {
  beforeEach(() => jest.clearAllMocks());

  it("loads and exposes stats correctly", async () => {
    mockGetCollectionCount.mockResolvedValueOnce(7);
    mockGetPlatformFee.mockResolvedValueOnce({
      bps: 250,
      receiver: "GRECEIVER",
    });
    function Comp() {
      const { stats, isLoading } = useLaunchpadAdminStats();
      if (isLoading || !stats) return <span data-testid="loading">yes</span>;
      return (
        <div>
          <span data-testid="collections">{stats.totalCollections}</span>
          <span data-testid="fee">{stats.platformFeeBps}</span>
          <span data-testid="receiver">{stats.platformFeeReceiver}</span>
        </div>
      );
    }
    render(<Comp />);
    await waitFor(() =>
      expect(screen.queryByTestId("loading")).not.toBeInTheDocument()
    );
    expect(screen.getByTestId("collections").textContent).toBe("7");
    expect(screen.getByTestId("fee").textContent).toBe("250");
    expect(screen.getByTestId("receiver").textContent).toBe("GRECEIVER");
  });

  it("sets error when contract call fails", async () => {
    mockGetCollectionCount.mockRejectedValueOnce(new Error("chain down"));
    function Comp() {
      const { error } = useLaunchpadAdminStats();
      return <span data-testid="error">{error ?? "none"}</span>;
    }
    render(<Comp />);
    await waitFor(() =>
      expect(screen.getByTestId("error").textContent).not.toBe("none")
    );
  });
});

// ── useLaunchpadAdminActions ──────────────────────────────────────────────────

describe("useLaunchpadAdminActions", () => {
  beforeEach(() => jest.clearAllMocks());

  it("transferAdmin returns false when adminPublicKey is null", async () => {
    function Comp() {
      const { transferAdmin } = useLaunchpadAdminActions(null);
      const [result, setResult] = React.useState<boolean | undefined>(undefined);
      return (
        <div>
          <button onClick={async () => setResult(await transferAdmin("GNEW"))}>
            transfer
          </button>
          <span data-testid="result">{String(result)}</span>
        </div>
      );
    }
    const user = userEvent.setup();
    render(<Comp />);
    await user.click(screen.getByRole("button"));
    await waitFor(() =>
      expect(screen.getByTestId("result").textContent).toBe("false")
    );
    expect(mockTransferLaunchpadAdmin).not.toHaveBeenCalled();
  });

  it("transferAdmin calls contract and returns true on success", async () => {
    mockTransferLaunchpadAdmin.mockResolvedValueOnce(undefined);
    function Comp() {
      const { transferAdmin } = useLaunchpadAdminActions("GADMIN");
      const [result, setResult] = React.useState<boolean | undefined>(undefined);
      return (
        <div>
          <button onClick={async () => setResult(await transferAdmin("GNEW"))}>
            transfer
          </button>
          <span data-testid="result">{String(result)}</span>
        </div>
      );
    }
    const user = userEvent.setup();
    render(<Comp />);
    await user.click(screen.getByRole("button"));
    await waitFor(() =>
      expect(screen.getByTestId("result").textContent).toBe("true")
    );
    expect(mockTransferLaunchpadAdmin).toHaveBeenCalledWith("GADMIN", "GNEW");
  });

  it("updateFee calls contract and returns true on success", async () => {
    mockUpdatePlatformFee.mockResolvedValueOnce(undefined);
    function Comp() {
      const { updateFee } = useLaunchpadAdminActions("GADMIN");
      const [result, setResult] = React.useState<boolean | undefined>(undefined);
      return (
        <div>
          <button onClick={async () => setResult(await updateFee("GRECEIVER", 500))}>
            update
          </button>
          <span data-testid="result">{String(result)}</span>
        </div>
      );
    }
    const user = userEvent.setup();
    render(<Comp />);
    await user.click(screen.getByRole("button"));
    await waitFor(() =>
      expect(screen.getByTestId("result").textContent).toBe("true")
    );
    expect(mockUpdatePlatformFee).toHaveBeenCalledWith("GADMIN", "GRECEIVER", 500);
  });

  it("updateFee sets error and returns false on contract failure", async () => {
    mockUpdatePlatformFee.mockRejectedValueOnce(new Error("tx failed"));
    function Comp() {
      const { updateFee, error } = useLaunchpadAdminActions("GADMIN");
      const [result, setResult] = React.useState<boolean | undefined>(undefined);
      return (
        <div>
          <button onClick={async () => setResult(await updateFee("GRECEIVER", 500))}>
            update
          </button>
          <span data-testid="result">{String(result)}</span>
          <span data-testid="error">{error ?? "none"}</span>
        </div>
      );
    }
    const user = userEvent.setup();
    render(<Comp />);
    await user.click(screen.getByRole("button"));
    await waitFor(() =>
      expect(screen.getByTestId("result").textContent).toBe("false")
    );
    expect(screen.getByTestId("error").textContent).not.toBe("none");
  });
});
