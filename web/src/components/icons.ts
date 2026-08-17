import type { LucideIcon } from "@lucide/svelte";
import ArrowUpDown from "@lucide/svelte/icons/arrow-up-down";
import Archive from "@lucide/svelte/icons/archive";
import Check from "@lucide/svelte/icons/check";
import ChevronDown from "@lucide/svelte/icons/chevron-down";
import Code from "@lucide/svelte/icons/code";
import Copy from "@lucide/svelte/icons/copy";
import Ellipsis from "@lucide/svelte/icons/ellipsis";
import Eraser from "@lucide/svelte/icons/eraser";
import FileText from "@lucide/svelte/icons/file-text";
import FileCode from "@lucide/svelte/icons/file-code";
import KeyRound from "@lucide/svelte/icons/key-round";
import Link from "@lucide/svelte/icons/link";
import Link2 from "@lucide/svelte/icons/link-2";
import List from "@lucide/svelte/icons/list";
import ListFilter from "@lucide/svelte/icons/list-filter";
import ListOrdered from "@lucide/svelte/icons/list-ordered";
import ListChecks from "@lucide/svelte/icons/list-checks";
import LogIn from "@lucide/svelte/icons/log-in";
import LogOut from "@lucide/svelte/icons/log-out";
import Minus from "@lucide/svelte/icons/minus";
import Monitor from "@lucide/svelte/icons/monitor";
import Moon from "@lucide/svelte/icons/moon";
import PanelLeftClose from "@lucide/svelte/icons/panel-left-close";
import PanelLeftOpen from "@lucide/svelte/icons/panel-left-open";
import PenLine from "@lucide/svelte/icons/pen-line";
import Plus from "@lucide/svelte/icons/plus";
import Printer from "@lucide/svelte/icons/printer";
import Quote from "@lucide/svelte/icons/quote";
import QrCode from "@lucide/svelte/icons/qr-code";
import Redo2 from "@lucide/svelte/icons/redo-2";
import Search from "@lucide/svelte/icons/search";
import ScrollText from "@lucide/svelte/icons/scroll-text";
import SlidersHorizontal from "@lucide/svelte/icons/sliders-horizontal";
import SquareCode from "@lucide/svelte/icons/square-code";
import Sun from "@lucide/svelte/icons/sun";
import Table2 from "@lucide/svelte/icons/table-2";
import TextAlignCenter from "@lucide/svelte/icons/text-align-center";
import TextAlignEnd from "@lucide/svelte/icons/text-align-end";
import TextAlignStart from "@lucide/svelte/icons/text-align-start";
import Trash2 from "@lucide/svelte/icons/trash-2";
import Undo2 from "@lucide/svelte/icons/undo-2";
import UserRound from "@lucide/svelte/icons/user-round";

export const icons = {
  "align-center": TextAlignCenter,
  "align-left": TextAlignStart,
  "align-right": TextAlignEnd,
  "arrow-up-down": ArrowUpDown,
  archive: Archive,
  check: Check,
  "chevron-down": ChevronDown,
  code: Code,
  copy: Copy,
  "edit-3": PenLine,
  eraser: Eraser,
  "file-code": FileCode,
  "file-text": FileText,
  "key-round": KeyRound,
  link: Link,
  "link-2": Link2,
  list: List,
  "list-filter": ListFilter,
  "list-ordered": ListOrdered,
  "list-checks": ListChecks,
  "log-in": LogIn,
  "log-out": LogOut,
  minus: Minus,
  monitor: Monitor,
  moon: Moon,
  "more-horizontal": Ellipsis,
  "panel-left-close": PanelLeftClose,
  "panel-left-open": PanelLeftOpen,
  plus: Plus,
  printer: Printer,
  "qr-code": QrCode,
  quote: Quote,
  "redo-2": Redo2,
  search: Search,
  "scroll-text": ScrollText,
  "sliders-horizontal": SlidersHorizontal,
  "square-code": SquareCode,
  sun: Sun,
  "table-2": Table2,
  "trash-2": Trash2,
  "undo-2": Undo2,
  "user-round": UserRound
} as const satisfies Record<string, LucideIcon>;

export type IconName = keyof typeof icons;
