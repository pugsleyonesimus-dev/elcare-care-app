import React from "react";
import { render, screen, fireEvent } from "@testing-library/react";
import { AdminConfirmationModal } from "@/components/AdminConfirmationModal";

describe("AdminConfirmationModal", () => {
  const defaultProps = {
    isOpen: true,
    onClose: jest.fn(),
    onConfirm: jest.fn(),
    title: "Test Action",
    actionDescription: "Are you sure you want to do this?",
    consequences: ["Bad things will happen", "You will lose money"],
  };

  it("should not render when closed", () => {
    render(<AdminConfirmationModal {...defaultProps} isOpen={false} />);
    expect(screen.queryByText("Test Action")).not.toBeInTheDocument();
  });

  it("should render title and description when open", () => {
    render(<AdminConfirmationModal {...defaultProps} />);
    expect(screen.getByText("Test Action")).toBeInTheDocument();
    expect(screen.getByText("Are you sure you want to do this?")).toBeInTheDocument();
  });

  it("should render consequences", () => {
    render(<AdminConfirmationModal {...defaultProps} />);
    expect(screen.getByText("Bad things will happen")).toBeInTheDocument();
    expect(screen.getByText("You will lose money")).toBeInTheDocument();
  });

  it("should call onConfirm when confirm button is clicked", () => {
    render(<AdminConfirmationModal {...defaultProps} />);
    fireEvent.click(screen.getByText("Confirm Action"));
    expect(defaultProps.onConfirm).toHaveBeenCalledTimes(1);
  });

  it("should call onClose when cancel button is clicked", () => {
    render(<AdminConfirmationModal {...defaultProps} />);
    fireEvent.click(screen.getByText("Cancel"));
    expect(defaultProps.onClose).toHaveBeenCalledTimes(1);
  });

  it("should show loading state when isProcessing is true", () => {
    render(<AdminConfirmationModal {...defaultProps} isProcessing={true} />);
    expect(screen.getByText("Processing...")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /processing/i })).toBeDisabled();
  });
});
