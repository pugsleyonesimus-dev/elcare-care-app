import React from 'react';
import { render, screen, fireEvent } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

jest.mock('lucide-react', () => ({
  AlertCircle: () => <span data-testid="icon-alert-circle" />,
  RefreshCw: () => <span data-testid="icon-refresh-cw" />,
  Package: () => <span data-testid="icon-package" />,
  SearchX: () => <span data-testid="icon-search-x" />,
}));

const MockLink = ({ children, href, className }: any) => (
  <a href={href} className={className} data-testid="next-link">
    {children}
  </a>
);
MockLink.displayName = 'MockLink';
jest.mock('next/link', () => MockLink);

import { ErrorState, EmptyState, NoResults } from '@/components/PageStates';

describe('ErrorState', () => {
  it('renders default title, message, and retry button', () => {
    render(<ErrorState message="Something went wrong" onRetry={jest.fn()} />);
    expect(screen.getByText('Failed to load')).toBeInTheDocument();
    expect(screen.getByText('Something went wrong')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /try again/i })).toBeInTheDocument();
    expect(screen.getByTestId('icon-alert-circle')).toBeInTheDocument();
    expect(screen.getByTestId('icon-refresh-cw')).toBeInTheDocument();
  });

  it('renders custom title when provided', () => {
    render(<ErrorState title="Custom error" message="Oops" onRetry={jest.fn()} />);
    expect(screen.getByText('Custom error')).toBeInTheDocument();
    expect(screen.queryByText('Failed to load')).not.toBeInTheDocument();
  });

  it('calls onRetry when Try Again button is clicked', async () => {
    const onRetry = jest.fn();
    const user = userEvent.setup();
    render(<ErrorState message="Error" onRetry={onRetry} />);
    await user.click(screen.getByRole('button', { name: /try again/i }));
    expect(onRetry).toHaveBeenCalledTimes(1);
  });

  it('applies custom className to container', () => {
    const { container } = render(
      <ErrorState message="Error" onRetry={jest.fn()} className="custom-class" />
    );
    const outer = container.firstChild as HTMLElement;
    expect(outer.className).toContain('custom-class');
  });
});

describe('EmptyState', () => {
  it('renders title and description', () => {
    render(<EmptyState title="No items" description="There are no items to show." />);
    expect(screen.getByText('No items')).toBeInTheDocument();
    expect(screen.getByText('There are no items to show.')).toBeInTheDocument();
    expect(screen.getByTestId('icon-package')).toBeInTheDocument();
  });

  it('renders without description when not provided', () => {
    render(<EmptyState title="No items" />);
    expect(screen.getByText('No items')).toBeInTheDocument();
    expect(screen.queryByRole('button')).not.toBeInTheDocument();
  });

  it('renders action button with onClick handler', async () => {
    const onClick = jest.fn();
    const user = userEvent.setup();
    render(
      <EmptyState
        title="Empty"
        action={{ label: 'Create Item', onClick }}
      />
    );
    const btn = screen.getByRole('button', { name: /create item/i });
    expect(btn).toBeInTheDocument();
    await user.click(btn);
    expect(onClick).toHaveBeenCalledTimes(1);
  });

  it('renders action as link when href is provided', () => {
    render(
      <EmptyState
        title="Empty"
        action={{ label: 'Go Home', href: '/' }}
      />
    );
    const link = screen.getByTestId('next-link');
    expect(link).toBeInTheDocument();
    expect(link).toHaveAttribute('href', '/');
    expect(link).toHaveTextContent('Go Home');
  });

  it('renders custom icon when icon prop is provided', () => {
    const CustomIcon = () => <span data-testid="custom-icon" />;
    render(<EmptyState title="Test" icon={CustomIcon} />);
    expect(screen.getByTestId('custom-icon')).toBeInTheDocument();
  });

  it('applies custom classNames', () => {
    render(
      <EmptyState
        title="Test"
        className="container-class"
        iconClassName="icon-class"
        titleClassName="title-class"
        descriptionClassName="desc-class"
        description="A description"
      />
    );
    expect(screen.getByText('Test').className).toContain('title-class');
    expect(screen.getByText('A description').className).toContain('desc-class');
  });
});

describe('NoResults', () => {
  it('renders default message and clear filters button', () => {
    render(<NoResults onClearFilters={jest.fn()} />);
    expect(screen.getByText('No results found')).toBeInTheDocument();
    expect(screen.getByText(/no artworks match/i)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /clear filters/i })).toBeInTheDocument();
    expect(screen.getByTestId('icon-search-x')).toBeInTheDocument();
  });

  it('renders custom message when provided', () => {
    render(
      <NoResults
        message="Try a different search."
        onClearFilters={jest.fn()}
      />
    );
    expect(screen.getByText('Try a different search.')).toBeInTheDocument();
    expect(screen.queryByText(/no artworks match/i)).not.toBeInTheDocument();
  });

  it('calls onClearFilters when Clear Filters button is clicked', async () => {
    const onClearFilters = jest.fn();
    const user = userEvent.setup();
    render(<NoResults onClearFilters={onClearFilters} />);
    await user.click(screen.getByRole('button', { name: /clear filters/i }));
    expect(onClearFilters).toHaveBeenCalledTimes(1);
  });

  it('applies custom className to container', () => {
    const { container } = render(
      <NoResults onClearFilters={jest.fn()} className="custom-no-results" />
    );
    const outer = container.firstChild as HTMLElement;
    expect(outer.className).toContain('custom-no-results');
  });
});
