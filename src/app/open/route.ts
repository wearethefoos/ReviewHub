import { NextResponse, type NextRequest } from "next/server";
import { parsePullRequestRef, prPath } from "@/lib/refs";

/** Resolves the header search box ("owner/repo#123" or a GitHub URL) to a ReviewHub page. */
export function GET(request: NextRequest) {
  const input = request.nextUrl.searchParams.get("pr") ?? "";
  const ref = parsePullRequestRef(input);
  const target = ref ? prPath(ref) : `/?error=${encodeURIComponent(input)}`;
  return NextResponse.redirect(new URL(target, request.url));
}
