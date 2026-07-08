import { commands } from "../types/generated";
import { unwrap } from "./client";

export const SearchApi = {
  // Defaults to 50 results to balance UI rendering performance with search comprehensiveness.
  unified: (query: string, limit: number = 50) =>
    unwrap(commands.unifiedSearch(query, limit)),
};
