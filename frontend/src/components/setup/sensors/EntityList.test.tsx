// @vitest-environment jsdom
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { checkSensor, getHaEntities, mapSensor } from "../../../api/client";
import { ApiError } from "../../../api/errors";
import type { EntityView } from "../../../types/sensors";
import EntityList from "./EntityList";

vi.mock("../../../api/client", () => ({
	getHaEntities: vi.fn(),
	checkSensor: vi.fn(),
	mapSensor: vi.fn(),
}));
const entities = vi.mocked(getHaEntities);
const check = vi.mocked(checkSensor);
const map = vi.mocked(mapSensor);

const entity = (over: Partial<EntityView>): EntityView => ({
	entity_id: "sensor.x",
	friendly_name: "X",
	value: 1,
	unit: null,
	device_class: null,
	reported_at: new Date(Date.now() - 4 * 60000).toISOString(),
	suggestion: null,
	mapped_to: null,
	...over,
});

const LIST = [
	entity({
		entity_id: "sensor.patio_temperature",
		friendly_name: "Patio temperature",
		value: 49.6,
		unit: "°F",
		device_class: "temperature",
		mapped_to: "air_temp",
	}),
	entity({
		entity_id: "sensor.shade_humidity",
		friendly_name: "Shade humidity",
		value: 64,
		unit: "%",
		device_class: "humidity",
		suggestion: "Humidity",
	}),
	entity({
		entity_id: "sensor.garden_probe",
		friendly_name: "Garden probe",
		value: 12.5,
		unit: "°C",
		device_class: "temperature",
	}),
	entity({
		entity_id: "sensor.fridge_power",
		friendly_name: "Fridge power",
		value: 120,
		unit: "W",
	}),
];

beforeEach(() => {
	entities.mockReset();
	check.mockReset();
	map.mockReset();
	entities.mockResolvedValue(LIST);
	map.mockResolvedValue([]);
	check.mockResolvedValue({
		value: 64,
		unit: "%",
		reported_at: new Date().toISOString(),
		otherwise: {
			value: 43,
			observed_at: new Date().toISOString(),
			basis: {
				kind: "Sensor",
				source: "Patio humidity",
				distance_miles: null,
				approximate: false,
			},
		},
		blocking: [],
		warnings: [],
	});
});

// @spec SETUP-MAP-001
describe("the entity list", () => {
	it("shows each numeric sensor's name, value, unit and last update, in the order given", async () => {
		render(<EntityList version={0} onMapped={vi.fn()} />);
		const rows = await screen.findAllByRole("listitem");
		expect(rows[0]).toHaveTextContent("Patio temperature");
		expect(rows[0]).toHaveTextContent("49.6 °F");
		expect(rows[0]).toHaveTextContent("4 min ago");
		expect(rows[3]).toHaveTextContent("Fridge power");
	});

	it("filters by text, by name or entity id", async () => {
		const user = userEvent.setup();
		render(<EntityList version={0} onMapped={vi.fn()} />);
		await screen.findAllByRole("listitem");
		await user.type(screen.getByLabelText("Filter sensors"), "probe");
		expect(screen.getAllByRole("listitem")).toHaveLength(1);
		await user.clear(screen.getByLabelText("Filter sensors"));
		await user.type(screen.getByLabelText("Filter sensors"), "fridge_power");
		expect(screen.getAllByRole("listitem")[0]).toHaveTextContent(
			"Fridge power",
		);
	});

	it("shows what an entity is already mapped to instead of offering to map it", async () => {
		render(<EntityList version={0} onMapped={vi.fn()} />);
		const patio = (await screen.findAllByRole("listitem"))[0];
		expect(patio).toHaveTextContent("Mapped to Air temperature");
		expect(within(patio).queryByRole("button", { name: "Map" })).toBeNull();
	});

	it("reads the list again when the mapped sensors change", async () => {
		const { rerender } = render(<EntityList version={0} onMapped={vi.fn()} />);
		await screen.findAllByRole("listitem");
		rerender(<EntityList version={1} onMapped={vi.fn()} />);
		await vi.waitFor(() => expect(entities).toHaveBeenCalledTimes(2));
	});

	it("shows why the list could not be read", async () => {
		entities.mockRejectedValue(
			new ApiError(503, "Home Assistant did not answer at that address."),
		);
		render(<EntityList version={0} onMapped={vi.fn()} />);
		expect(
			await screen.findByText("Home Assistant did not answer at that address."),
		).toBeInTheDocument();
	});
});

// @spec SETUP-MAP-002, SETUP-MAP-003, SETUP-MAP-004
describe("mapping an entity", () => {
	it("opens in place with the entity's name and its suggested input, then checks it", async () => {
		const user = userEvent.setup();
		render(<EntityList version={0} onMapped={vi.fn()} />);
		const shade = (await screen.findAllByRole("listitem"))[1];
		await user.click(within(shade).getByRole("button", { name: "Map" }));
		expect(within(shade).getByLabelText("Name")).toHaveValue("Shade humidity");
		expect(within(shade).getByLabelText("Input")).toHaveValue("humidity");
		expect(check).toHaveBeenCalledWith("sensor.shade_humidity", "humidity");
		expect(
			await within(shade).findByText(
				"Your sensor 64% · Your sensor · Patio humidity 43%",
			),
		).toBeInTheDocument();
	});

	it("asks a thermometer whether it is in the air or the soil before checking it", async () => {
		const user = userEvent.setup();
		render(<EntityList version={0} onMapped={vi.fn()} />);
		const probe = (await screen.findAllByRole("listitem"))[2];
		await user.click(within(probe).getByRole("button", { name: "Map" }));
		expect(within(probe).getByLabelText("Input")).toHaveValue("");
		expect(check).not.toHaveBeenCalled();
		expect(within(probe).getByRole("button", { name: "Save" })).toBeDisabled();
		await user.selectOptions(
			within(probe).getByLabelText("Input"),
			"soil_temp_10",
		);
		expect(check).toHaveBeenCalledWith("sensor.garden_probe", "soil_temp_10");
	});
});

// @spec SETUP-MAP-005, SETUP-MAP-006
describe("what the check finds", () => {
	it("blocks the save for a wrong unit or an old reading", async () => {
		const user = userEvent.setup();
		check.mockResolvedValue({
			value: null,
			unit: "W",
			reported_at: null,
			otherwise: null,
			blocking: [
				"This sensor reports in W. For air temperature TurfOps accepts °F or °C.",
			],
			warnings: [],
		});
		render(<EntityList version={0} onMapped={vi.fn()} />);
		const fridge = (await screen.findAllByRole("listitem"))[3];
		await user.click(within(fridge).getByRole("button", { name: "Map" }));
		await user.selectOptions(
			within(fridge).getByLabelText("Input"),
			"air_temp",
		);
		expect(await within(fridge).findByText(/reports in W/)).toBeInTheDocument();
		expect(within(fridge).getByRole("button", { name: "Save" })).toBeDisabled();
	});

	it("warns of an implausible value and still lets the owner save", async () => {
		const user = userEvent.setup();
		check.mockResolvedValue({
			value: 140,
			unit: "%",
			reported_at: new Date().toISOString(),
			otherwise: null,
			blocking: [],
			warnings: [
				"140.0 is outside what humidity can be. Check that this is the right sensor.",
			],
		});
		render(<EntityList version={0} onMapped={vi.fn()} />);
		const shade = (await screen.findAllByRole("listitem"))[1];
		await user.click(within(shade).getByRole("button", { name: "Map" }));
		expect(
			await within(shade).findByText(/outside what humidity can be/),
		).toBeInTheDocument();
		expect(within(shade).getByRole("button", { name: "Save" })).toBeEnabled();
	});
});

// @spec SETUP-MAP-007, SETUP-MAP-003
describe("saving a mapping", () => {
	it("maps the entity under the name given", async () => {
		const user = userEvent.setup();
		const onMapped = vi.fn();
		render(<EntityList version={0} onMapped={onMapped} />);
		const shade = (await screen.findAllByRole("listitem"))[1];
		await user.click(within(shade).getByRole("button", { name: "Map" }));
		await within(shade).findByText(/Your sensor 64%/);
		await user.click(within(shade).getByRole("button", { name: "Save" }));
		expect(map).toHaveBeenCalledWith({
			name: "Shade humidity",
			entity_id: "sensor.shade_humidity",
			input: "humidity",
		});
		expect(onMapped).toHaveBeenCalled();
	});

	it("asks in place before replacing the sensor an input already has, naming both", async () => {
		const user = userEvent.setup();
		map.mockRejectedValueOnce(
			new ApiError(
				409,
				"Patio humidity already supplies humidity. Replace it with Shade humidity?",
			),
		);
		render(<EntityList version={0} onMapped={vi.fn()} />);
		const shade = (await screen.findAllByRole("listitem"))[1];
		await user.click(within(shade).getByRole("button", { name: "Map" }));
		await within(shade).findByText(/Your sensor 64%/);
		await user.click(within(shade).getByRole("button", { name: "Save" }));
		expect(
			await within(shade).findByText(
				"Patio humidity already supplies humidity. Replace it with Shade humidity?",
			),
		).toBeInTheDocument();
		await user.click(within(shade).getByRole("button", { name: "Replace" }));
		expect(map).toHaveBeenLastCalledWith({
			name: "Shade humidity",
			entity_id: "sensor.shade_humidity",
			input: "humidity",
			replace: true,
		});
	});
});
