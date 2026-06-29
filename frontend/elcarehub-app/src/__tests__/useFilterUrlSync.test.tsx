/**
 * Tests for useFilterUrlSync hook — URL <-> filter state sync.
 */
import React from "react";
import { render, screen } from "@testing-library/react";
import { useFilterUrlSync, FilterUrlSync } from "@/hooks/useFilterUrlSync";
import type { Filters } from "@/components/SearchFilter";

// ── Mocks ──────────────────────────────────────────────────

const mockReplace = jest.fn();

let mockSearchParams = new URLSearchParams();
let mockPathname = "/explore";

jest.mock("next/navigation", () => ({
  useRouter: () => ({ replace: mockReplace }),
  useSearchParams: () => mockSearchParams,
  usePathname: () => mockPathname,
}));

beforeEach(() => {
  mockReplace.mockClear();
  mockSearchParams = new URLSearchParams();
  mockPathname = "/explore";
});

// ── Helper component to exercise the hook ─────────────────

function TestConsumer({
  onHook,
}: {
  onHook: (api: FilterUrlSync) => void;
}) {
  const api = useFilterUrlSync();
  // Give the parent a chance to inspect / call the hook
  React.useEffect(() => {
    onHook(api);
  }, [api, onHook]);
  return <div data-testid="consumer">ok</div>;
}

function renderConsumer() {
  let api!: FilterUrlSync;
  const onHook = jest.fn((a: FilterUrlSync) => {
    api = a;
  });
  render(<TestConsumer onHook={onHook} />);
  return { api: () => api, onHook };
}

// ── Tests ──────────────────────────────────────────────────

describe("useFilterUrlSync", () => {
  // ── Reading initial state from URL ─────────────────────

  it("reads default filter values when URL has no params", () => {
    const { api } = renderConsumer();
    const { initialFilters } = api();
    expect(initialFilters).toEqual<Filters>({
      search: "",
      status: "All",
      category: "All",
      minPrice: "",
      maxPrice: "",
      sort: "newest",
    });
  });

  it("reads initial page 1 when no page param", () => {
    const { api } = renderConsumer();
    expect(api().initialPage).toBe(1);
  });

  it("parses search from ?q= param", () => {
    mockSearchParams = new URLSearchParams("q=landscape");
    const { api } = renderConsumer();
    expect(api().initialFilters.search).toBe("landscape");
  });

  it("parses status from ?status= param", () => {
    mockSearchParams = new URLSearchParams("status=Active");
    const { api } = renderConsumer();
    expect(api().initialFilters.status).toBe("Active");
  });

  it("parses category from ?category= param", () => {
    mockSearchParams = new URLSearchParams("category=Painting");
    const { api } = renderConsumer();
    expect(api().initialFilters.category).toBe("Painting");
  });

  it("parses price range from ?minPrice=&maxPrice= params", () => {
    mockSearchParams = new URLSearchParams("minPrice=10&maxPrice=100");
    const { api } = renderConsumer();
    expect(api().initialFilters.minPrice).toBe("10");
    expect(api().initialFilters.maxPrice).toBe("100");
  });

  it("parses sort from ?sort= param", () => {
    mockSearchParams = new URLSearchParams("sort=price-low");
    const { api } = renderConsumer();
    expect(api().initialFilters.sort).toBe("price-low");
  });

  it("parses page from ?page= param", () => {
    mockSearchParams = new URLSearchParams("page=3");
    const { api } = renderConsumer();
    expect(api().initialPage).toBe(3);
  });

  it("falls back to page 1 when ?page= is invalid", () => {
    mockSearchParams = new URLSearchParams("page=abc");
    const { api } = renderConsumer();
    expect(api().initialPage).toBe(1);
  });

  it("falls back to page 1 when ?page= is 0", () => {
    mockSearchParams = new URLSearchParams("page=0");
    const { api } = renderConsumer();
    expect(api().initialPage).toBe(1);
  });

  it("reads all combined params", () => {
    mockSearchParams = new URLSearchParams(
      "q=portrait&status=Active&category=Photography&minPrice=5&maxPrice=50&sort=price-high&page=2",
    );
    const { api } = renderConsumer();
    expect(api().initialFilters).toEqual<Filters>({
      search: "portrait",
      status: "Active",
      category: "Photography",
      minPrice: "5",
      maxPrice: "50",
      sort: "price-high",
    });
    expect(api().initialPage).toBe(2);
  });

  // ── syncToUrl ──────────────────────────────────────────

  it("syncToUrl calls router.replace with correct params", () => {
    const { api } = renderConsumer();
    const filters: Filters = {
      search: "landscape",
      status: "Active",
      category: "All",
      minPrice: "10",
      maxPrice: "",
      sort: "newest",
    };
    api().syncToUrl(filters, 1);
    expect(mockReplace).toHaveBeenCalledWith(
      "/explore?q=landscape&status=Active&minPrice=10",
      { scroll: false },
    );
  });

  it("syncToUrl omits default values from the URL", () => {
    const { api } = renderConsumer();
    const filters: Filters = {
      search: "",
      status: "All",
      category: "All",
      minPrice: "",
      maxPrice: "",
      sort: "newest",
    };
    api().syncToUrl(filters, 1);
    // No params = pathname only
    expect(mockReplace).toHaveBeenCalledWith("/explore", {
      scroll: false,
    });
  });

  it("syncToUrl includes page when > 1", () => {
    const { api } = renderConsumer();
    api().syncToUrl(
      { search: "", status: "All", category: "All", minPrice: "", maxPrice: "", sort: "newest" },
      3,
    );
    expect(mockReplace).toHaveBeenCalledWith("/explore?page=3", {
      scroll: false,
    });
  });

  it("syncToUrl includes sort when non-default", () => {
    const { api } = renderConsumer();
    const filters: Filters = {
      search: "",
      status: "All",
      category: "All",
      minPrice: "",
      maxPrice: "",
      sort: "price-low",
    };
    api().syncToUrl(filters, 1);
    expect(mockReplace).toHaveBeenCalledWith("/explore?sort=price-low", {
      scroll: false,
    });
  });

  it("syncToUrl includes category when non-default", () => {
    const { api } = renderConsumer();
    const filters: Filters = {
      search: "",
      status: "All",
      category: "Sculpture",
      minPrice: "",
      maxPrice: "",
      sort: "newest",
    };
    api().syncToUrl(filters, 1);
    expect(mockReplace).toHaveBeenCalledWith("/explore?category=Sculpture", {
      scroll: false,
    });
  });

  it("syncToUrl uses the current pathname", () => {
    mockPathname = "/explore/special";
    const { api } = renderConsumer();
    const filters: Filters = {
      search: "test",
      status: "All",
      category: "All",
      minPrice: "",
      maxPrice: "",
      sort: "newest",
    };
    api().syncToUrl(filters, 1);
    expect(mockReplace).toHaveBeenCalledWith(
      "/explore/special?q=test",
      { scroll: false },
    );
  });
});
