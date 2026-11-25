// @vitest-environment jsdom
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { describe, expect, it } from "vitest";
import SetupNotice from "./SetupNotice";

// @spec SETUP-FLOW-001
describe("SetupNotice", () => {
	it("asks for the lawn's location, links to setup and says dates use UTC until then", () => {
		render(
			<MemoryRouter>
				<SetupNotice />
			</MemoryRouter>,
		);
		const title = screen.getByRole("heading", {
			name: "Tell TurfOps where your lawn is",
		});
		expect(title.parentElement).toHaveTextContent(
			"Until it is set, dates use UTC.",
		);
		expect(
			screen.getByRole("link", { name: "Set your location" }),
		).toHaveAttribute("href", "/setup?step=location");
	});
});
