import React from "react";
import { render, screen, fireEvent } from "@testing-library/react";
import { WalletMenu } from "@/components/WalletMenu";

describe("WalletMenu", () => {
  const defaultProps = {
    address: "GDOW...WXYZ",
    balance: "123.456",
    isLoadingBalance: false,
    onDisconnect: jest.fn(),
  };

  it("renders truncated address and balance", () => {
    render(<WalletMenu {...defaultProps} />);
    expect(screen.getByText("GDOW...WXYZ")).toBeInTheDocument();
    expect(screen.getByText("123.456")).toBeInTheDocument();
  });

  it("shows loading state when fetching balance", () => {
    render(<WalletMenu {...defaultProps} isLoadingBalance={true} />);
    expect(screen.getByText("Fetching...")).toBeInTheDocument();
  });

  it("calls onDisconnect when disconnect button is clicked", () => {
    render(<WalletMenu {...defaultProps} />);
    fireEvent.click(screen.getByText("Disconnect Wallet"));
    expect(defaultProps.onDisconnect).toHaveBeenCalled();
  });

  it("copies address to clipboard", async () => {
    // Mock clipboard
    Object.assign(navigator, {
      clipboard: {
        writeText: jest.fn().mockImplementation(() => Promise.resolve()),
      },
    });

    render(<WalletMenu {...defaultProps} address="FULL_ADDRESS_HERE" />);
    fireEvent.click(screen.getByTitle("Copy Address"));
    
    expect(navigator.clipboard.writeText).toHaveBeenCalledWith("FULL_ADDRESS_HERE");
  });
});
