import { Card, PageHeader } from "../components/ui";

export default function TerminalPage() {
  return (
    <>
      <PageHeader title="Terminal" subtitle="Integrated SSH terminal." />
      <Card className="font-mono text-sm">
        <p className="text-muted">$ ssh integration</p>
        <p className="mt-2">SSH terminal support is coming in a later milestone.</p>
        <p className="mt-1 text-muted">No connections are made by this application yet.</p>
      </Card>
    </>
  );
}
