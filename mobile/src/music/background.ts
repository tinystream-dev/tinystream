// SPDX-License-Identifier: AGPL-3.0-or-later
import { AppRegistry } from "react-native";
AppRegistry.registerHeadlessTask(
  "TinystreamMusic",
  () => () => new Promise<void>(() => {}),
);
