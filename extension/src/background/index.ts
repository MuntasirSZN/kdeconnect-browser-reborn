/**
 * Background service worker — wires the port, menus, and options modules.
 */

import "./menus";
import "./port";
import "./options";

import { loadOptions } from "./options";

loadOptions();
