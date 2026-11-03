import { Badge, PageHeader, Table } from "../components/ui";
import { containerTone } from "../components/tones";
import { mockContainers } from "../demo/demoData";

export default function DockerPage() {
  return (
    <>
      <PageHeader title="Docker" subtitle="Container status (read-only preview)." mock />
      <Table headers={["Name", "Image", "State", "Ports", "Uptime"]}>
        {mockContainers.map((c) => (
          <tr key={c.id}>
            <td className="px-4 py-3 font-medium">{c.name}</td>
            <td className="px-4 py-3 text-muted">{c.image}</td>
            <td className="px-4 py-3">
              <Badge tone={containerTone[c.state]}>{c.state}</Badge>
            </td>
            <td className="px-4 py-3">{c.ports}</td>
            <td className="px-4 py-3 text-muted">{c.uptime}</td>
          </tr>
        ))}
      </Table>
    </>
  );
}
