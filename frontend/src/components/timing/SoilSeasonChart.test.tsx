// @vitest-environment jsdom
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";
import type { SoilSeriesDay } from "../../types/timing";
import SoilSeasonChart from "./SoilSeasonChart";
import { seriesFor } from "./testFixtures";

// jsdom has no layout, so Recharts draws nothing; stand-ins expose the props instead.
vi.mock("recharts", () => ({
	ResponsiveContainer: ({ children }: { children: ReactNode }) => (
		<div>{children}</div>
	),
	LineChart: ({ data, children }: { data: unknown[]; children: ReactNode }) => (
		<div data-testid="chart" data-rows={JSON.stringify(data)}>
			{children}
		</div>
	),
	Line: (p: { dataKey: string; stroke: string; strokeDasharray?: string }) => (
		<div
			data-testid="line"
			data-key={p.dataKey}
			data-stroke={p.stroke}
			data-dash={p.strokeDasharray ?? ""}
		/>
	),
	YAxis: (p: { ticks: number[] }) => (
		<div data-testid="yaxis" data-ticks={JSON.stringify(p.ticks)} />
	),
	ReferenceLine: (p: { y?: number; x?: string; label?: { value: string } }) =>
		p.y !== undefined ? (
			<div data-testid="threshold" data-y={p.y} />
		) : (
			<div data-testid="xline" data-x={p.x} data-label={p.label?.value} />
		),
	XAxis: () => null,
	CartesianGrid: () => null,
	Tooltip: () => null,
}));

type Row = SoilSeriesDay & { label: string };

function rows(): Row[] {
	return JSON.parse(screen.getByTestId("chart").dataset.rows ?? "[]");
}

function line(key: string): DOMStringMap {
	const el = screen.getAllByTestId("line").find((l) => l.dataset.key === key);
	if (!el) throw new Error(`no ${key} line`);
	return el.dataset;
}

function thresholds(): number[] {
	return screen.getAllByTestId("threshold").map((t) => Number(t.dataset.y));
}

/** Observed through Sep 19 at 66°F, forecast Sep 20–24 at 64°F. */
function fallSeries(): SoilSeriesDay[] {
	return seriesFor(2026).map((d) => ({
		...d,
		this_year: d.date <= "2026-09-19" ? 66 : null,
		forecast: d.date >= "2026-09-20" && d.date <= "2026-09-24" ? 64 : null,
	}));
}

// @spec TIME-UI-006
describe("SoilSeasonChart", () => {
	it("draws this year solid blue, the forecast dashed in the same hue, last year orange, the typical year a thin neutral line", () => {
		render(<SoilSeasonChart series={fallSeries()} today="2026-09-20" />);
		expect(line("this_measured")).toMatchObject({
			stroke: "#2a78d6",
			dash: "",
		});
		expect(line("forecast").stroke).toBe("#2a78d6");
		expect(line("forecast").dash).not.toBe("");
		expect(line("last_measured")).toMatchObject({
			stroke: "#eb6834",
			dash: "",
		});
		expect(line("typical")).toMatchObject({ stroke: "#718096", dash: "" });
	});

	it("dots estimated stretches in their own hue, joined to the measured days, and marks only lasting changes", () => {
		const series = fallSeries().map((d) => ({
			...d,
			this_year_estimated: d.date >= "2026-09-10" && d.date <= "2026-09-12",
			last_year_estimated: d.date === "2026-09-01",
		}));
		render(
			<SoilSeasonChart
				series={series}
				today="2026-09-20"
				changes={["2026-09-10", "2026-01-15"]}
			/>,
		);
		expect(line("this_estimated")).toMatchObject({ stroke: "#2a78d6" });
		expect(line("this_estimated").dash).not.toBe("");
		expect(line("last_estimated")).toMatchObject({ stroke: "#eb6834" });
		expect(line("last_estimated").dash).not.toBe("");
		const byDate = new Map(rows().map((r) => [r.date, r]));
		const at = (date: string) =>
			byDate.get(date) as unknown as Record<string, number | null>;
		expect(at("2026-09-11").this_measured).toBeNull();
		expect(at("2026-09-11").this_estimated).toBe(66);
		// The measured days on either side carry the dotted segment's ends.
		expect(at("2026-09-09").this_estimated).toBe(66);
		expect(at("2026-09-13").this_estimated).toBe(66);
		expect(at("2026-09-14").this_estimated).toBeNull();
		const markers = screen
			.getAllByTestId("xline")
			.filter((x) => x.dataset.label === "Source changed");
		// Only the change inside the plotted range is drawn.
		expect(markers.map((m) => m.dataset.x)).toEqual(["Sep 10"]);
		expect(
			screen.getByText("Dotted = estimated for your location"),
		).toBeInTheDocument();
	});

	// @spec UI-CONV-004
	it("explains the Today marker in the legend", () => {
		render(<SoilSeasonChart series={fallSeries()} today="2026-09-20" />);
		const legend = screen
			.getAllByText("Typical year")
			.map((key) => key.parentElement as HTMLElement)
			.find((row) => within(row).queryByText("Last year"));
		expect(legend).toBeDefined();
		expect(
			within(legend as HTMLElement).getByText("Today"),
		).toBeInTheDocument();
	});

	it("joins the forecast to the last observed day", () => {
		render(<SoilSeasonChart series={fallSeries()} today="2026-09-20" />);
		const sep19 = rows().find((r) => r.date === "2026-09-19");
		expect(sep19).toMatchObject({ this_year: 66, forecast: 66 });
		const sep18 = rows().find((r) => r.date === "2026-09-18");
		expect(sep18?.forecast).toBeNull();
	});

	it("uses 10°F ticks spanning the plotted values", () => {
		render(<SoilSeasonChart series={fallSeries()} today="2026-09-20" />);
		const ticks: number[] = JSON.parse(
			screen.getByTestId("yaxis").dataset.ticks ?? "[]",
		);
		expect(ticks).toEqual([50, 60, 70]);
	});

	it("shows the fall thresholds from July and the spring thresholds before", () => {
		const { unmount } = render(
			<SoilSeasonChart series={fallSeries()} today="2026-07-01" />,
		);
		expect(thresholds()).toEqual([75, 70, 65, 55]);
		unmount();
		render(<SoilSeasonChart series={fallSeries()} today="2026-06-30" />);
		expect(thresholds()).toEqual([45, 50, 55, 65]);
	});

	it("shows 75 days before to 60 days after today, with a full-year toggle", async () => {
		render(<SoilSeasonChart series={fallSeries()} today="2026-09-20" />);
		const season = rows();
		expect(season[0].date).toBe("2026-07-07");
		expect(season[season.length - 1].date).toBe("2026-11-19");

		await userEvent.click(
			screen.getByRole("button", { name: "Show full year" }),
		);
		expect(rows()).toHaveLength(365);
		expect(
			screen.getByRole("button", { name: "Show this season" }),
		).toHaveAttribute("aria-pressed", "true");
	});

	it("has a table view of the same days", () => {
		render(<SoilSeasonChart series={fallSeries()} today="2026-09-20" />);
		expect(screen.getByText("View as table")).toBeInTheDocument();
		const table = screen.getByRole("table", { hidden: true });
		expect(table).toHaveTextContent("Sep 19");
		expect(table).toHaveTextContent("66.0°F");
		expect(table).toHaveTextContent("64.0°F (forecast)");
	});
});
