/**
 * Component tests for SearchFilter.
 */
import React from 'react';
import { render, screen, fireEvent } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

// lucide-react icons can cause issues in jsdom — mock them
jest.mock('lucide-react', () => ({
  Search: () => <span data-testid="icon-search" />,
  SlidersHorizontal: () => <span />,
  ArrowUpDown: () => <span />,
  X: () => <span data-testid="icon-x" />,
  Filter: () => <span />,
}));

import { SearchFilter } from '@/components/SearchFilter';
import type { Filters } from '@/components/SearchFilter';

const DEFAULT_FILTERS: Filters = {
  search: '',
  status: 'All',
  category: 'All',
  minPrice: '',
  maxPrice: '',
  sort: 'newest',
};

describe('SearchFilter', () => {
  it('renders the search input', () => {
    const onChange = jest.fn();
    render(
      <SearchFilter
        filters={DEFAULT_FILTERS}
        onFilterChange={onChange}
        showFilters={false}
        setShowFilters={jest.fn()}
        totalResults={5}
      />
    );
    expect(screen.getByPlaceholderText(/search/i)).toBeInTheDocument();
  });

  it('calls onFilterChange with updated search text', async () => {
    const onChange = jest.fn();
    const user = userEvent.setup();
    render(
      <SearchFilter
        filters={DEFAULT_FILTERS}
        onFilterChange={onChange}
        showFilters={false}
        setShowFilters={jest.fn()}
        totalResults={5}
      />
    );
    await user.type(screen.getByPlaceholderText(/search/i), 'landscape');
    expect(onChange).toHaveBeenCalledWith({ search: 'l' });
  });

  it('calls setShowFilters when filter toggle button is clicked', async () => {
    const setShowFilters = jest.fn();
    const user = userEvent.setup();
    render(
      <SearchFilter
        filters={DEFAULT_FILTERS}
        onFilterChange={jest.fn()}
        showFilters={false}
        setShowFilters={setShowFilters}
        totalResults={0}
      />
    );
    // Click the "Filters" toggle button
    const filterBtn = screen.getByRole('button', { name: /filter/i });
    await user.click(filterBtn);
    expect(setShowFilters).toHaveBeenCalledWith(true);
  });

  it('shows status filter buttons when showFilters is true', () => {
    render(
      <SearchFilter
        filters={DEFAULT_FILTERS}
        onFilterChange={jest.fn()}
        showFilters={true}
        setShowFilters={jest.fn()}
        totalResults={0}
      />
    );
    expect(screen.getByRole('button', { name: /active/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /sold/i })).toBeInTheDocument();
  });

  it('calls onFilterChange with status when status button is clicked', async () => {
    const onChange = jest.fn();
    const user = userEvent.setup();
    render(
      <SearchFilter
        filters={DEFAULT_FILTERS}
        onFilterChange={onChange}
        showFilters={true}
        setShowFilters={jest.fn()}
        totalResults={0}
      />
    );
    await user.click(screen.getByRole('button', { name: /^active$/i }));
    expect(onChange).toHaveBeenCalledWith({ status: 'Active' });
  });

  it('shows the total results count', () => {
    render(
      <SearchFilter
        filters={DEFAULT_FILTERS}
        onFilterChange={jest.fn()}
        showFilters={false}
        setShowFilters={jest.fn()}
        totalResults={42}
      />
    );
    expect(screen.getByText(/42/)).toBeInTheDocument();
  });

  it('shows Reset All Filters button when filters are active', async () => {
    const onChange = jest.fn();
    const user = userEvent.setup();
    render(
      <SearchFilter
        filters={{ ...DEFAULT_FILTERS, search: 'test' }}
        onFilterChange={onChange}
        showFilters={false}
        setShowFilters={jest.fn()}
        totalResults={0}
      />
    );
    const clearBtn = screen.getByRole('button', { name: /reset all filters/i });
    expect(clearBtn).toBeInTheDocument();
    await user.click(clearBtn);
    expect(onChange).toHaveBeenCalledWith(expect.objectContaining({ search: '' }));
  });

  it('does not show Reset All Filters button when no filters are active', () => {
    render(
      <SearchFilter
        filters={DEFAULT_FILTERS}
        onFilterChange={jest.fn()}
        showFilters={false}
        setShowFilters={jest.fn()}
        totalResults={0}
      />
    );
    expect(screen.queryByRole('button', { name: /reset all filters/i })).not.toBeInTheDocument();
  });

  // ── URL-sync related tests ───────────────────────────────

  it('renders with filters initialized from URL params', () => {
    const urlInitFilters: Filters = {
      search: 'portrait',
      status: 'Active',
      category: 'Photography',
      minPrice: '5',
      maxPrice: '50',
      sort: 'price-high',
    };
    render(
      <SearchFilter
        filters={urlInitFilters}
        onFilterChange={jest.fn()}
        showFilters={true}
        setShowFilters={jest.fn()}
        totalResults={10}
      />
    );
    const searchInput = screen.getByPlaceholderText(/search/i) as HTMLInputElement;
    expect(searchInput.value).toBe('portrait');
    // Status button should reflect the active status
    const activeBtn = screen.getByRole('button', { name: /^active$/i });
    expect(activeBtn.className).toContain('bg-brand');
    // Category select
    const categorySelect = screen.getByDisplayValue('Photography');
    expect(categorySelect).toBeInTheDocument();
    // Price inputs
    const priceInputs = screen.getAllByRole('spinbutton');
    expect(priceInputs[0]).toHaveValue(5);
    expect(priceInputs[1]).toHaveValue(50);
  });

  it('preserves URL-derived search value across renders', () => {
    const onChange = jest.fn();
    const { rerender } = render(
      <SearchFilter
        filters={{ ...DEFAULT_FILTERS, search: 'abstract' }}
        onFilterChange={onChange}
        showFilters={false}
        setShowFilters={jest.fn()}
        totalResults={3}
      />
    );
    const input = screen.getByPlaceholderText(/search/i) as HTMLInputElement;
    expect(input.value).toBe('abstract');

    // Simulate a re-render with updated filters (as would happen
    // when a debounced value catches up)
    rerender(
      <SearchFilter
        filters={{ ...DEFAULT_FILTERS, search: 'abstract art' }}
        onFilterChange={onChange}
        showFilters={false}
        setShowFilters={jest.fn()}
        totalResults={3}
      />
    );
    expect(input.value).toBe('abstract art');
  });

  it('displays the correct total results after URL-synced filtering', () => {
    render(
      <SearchFilter
        filters={DEFAULT_FILTERS}
        onFilterChange={jest.fn()}
        showFilters={false}
        setShowFilters={jest.fn()}
        totalResults={99}
      />
    );
    expect(screen.getByText(/99/)).toBeInTheDocument();
    expect(screen.getByText(/99/).className).toContain('font-bold');
  });
});
