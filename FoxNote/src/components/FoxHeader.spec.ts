import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import FoxHeader from "./FoxHeader.vue";

describe("FoxHeader", () => {
  it("renders top bar and emits actions", async () => {
    const wrapper = mount(FoxHeader, {
      props: {
        title: "root",
        subtitle: "2 notes",
        activeTagLabel: "work/project (3)",
        searchQuery: "physics",
      },
      global: {
        stubs: {
          VBtn: {
            emits: ["click"],
            template: `<button @click="$emit('click')"><slot/></button>`,
          },
          VTextField: {
            emits: ["update:modelValue"],
            props: ["modelValue"],
            template: `<input :value="modelValue" @input="$emit('update:modelValue', $event.target.value)" />`,
          },
        },
      },
    });

    expect(wrapper.text()).toContain("root");
    expect(wrapper.text()).toContain("2 notes");
    expect(wrapper.text()).toContain("work/project (3)");

    await wrapper.find("input").setValue("new query");
    expect(wrapper.emitted("updateSearchQuery")?.[0]).toEqual(["new query"]);

    const buttons = wrapper.findAll("button");
    await buttons[0]?.trigger("click");
    await buttons[1]?.trigger("click");

    expect(wrapper.emitted("clearTagFilter")).toHaveLength(1);
    expect(wrapper.emitted("refresh")).toHaveLength(1);
  });
});
