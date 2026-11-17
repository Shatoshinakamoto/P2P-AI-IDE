/**
 * Tip.tsx -- a hover hint (Radix Tooltip) for buttons in the activity bar and status bar.
 * Text only, rendered by React (escaped), so a hint can never inject markup (CWE-79).
 */
import * as Tooltip from "@radix-ui/react-tooltip";
import type { ReactNode } from "react";

/** Wraps one focusable child with a hint shown on hover and keyboard focus. */
export function Tip({ text, side = "top", children }: { text: string; side?: "top" | "right" | "bottom" | "left"; children: ReactNode }) {
  return (
    <Tooltip.Root delayDuration={300}>
      <Tooltip.Trigger asChild>{children}</Tooltip.Trigger>
      <Tooltip.Portal>
        <Tooltip.Content
          side={side}
          sideOffset={6}
          className="z-50 max-w-xs rounded border border-ide-line bg-[#252526] px-2 py-1 text-xs text-ide-bright shadow-lg"
        >
          {text}
        </Tooltip.Content>
      </Tooltip.Portal>
    </Tooltip.Root>
  );
}
