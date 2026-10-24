import { describe, expect, it } from "vitest";
import { validateRubric } from "./rubric";

const point = (id: string, over = {}) => ({ id, point: `point ${id}`, weight: 1, essential: false, ...over });

describe("validateRubric", () => {
  it("valid_rubric_has_no_errors", () => {
    expect(validateRubric([point("r1"), point("r2", { essential: true })], 0.6)).toEqual([]);
    expect(validateRubric([point("r1")], 1)).toEqual([]);
  });

  it("rejects_empty_duplicate_zero_weight_and_bad_threshold", () => {
    expect(validateRubric([], 0.6)).toContain("The rubric needs at least one point.");
    expect(validateRubric([point("r1"), point("r1")], 0.6)).toContain('Point id "r1" is used twice.');
    expect(validateRubric([point(" ")], 0.6)).toContain("Every point needs an id.");
    expect(validateRubric([point("r1", { point: "  " })], 0.6)).toContain("Point r1 has no text.");
    expect(validateRubric([point("r1", { weight: 0 })], 0.6)).toContain("Point r1 needs a weight above 0.");
    expect(validateRubric([point("r1")], 0)).toContain(
      "The pass threshold must be above 0% and at most 100%.",
    );
    expect(validateRubric([point("r1")], 1.2)).toContain(
      "The pass threshold must be above 0% and at most 100%.",
    );
  });
});
