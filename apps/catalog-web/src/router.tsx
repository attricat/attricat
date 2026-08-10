import { useQuery } from '@tanstack/react-query'
import {
  Link,
  Outlet,
  createRootRoute,
  createRoute,
  createRouter,
  useNavigate,
} from '@tanstack/react-router'
import { flexRender } from '@tanstack/react-table'
import { getCoreRowModel, legacyCreateColumnHelper, type LegacyColumnDef, useLegacyTable } from '@tanstack/react-table/legacy'
import { useState, type FormEvent } from 'react'
import { getEntityPreview, searchEntities, type EntityItem } from './api'
import { displayValue, parseExplorerSearch } from './search'

const rootRoute = createRootRoute({ component: () => <Outlet /> })

const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/',
  validateSearch: parseExplorerSearch,
  component: Explorer,
})

const previewRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/entities/$entityId',
  component: EntityPreview,
})

const routeTree = rootRoute.addChildren([indexRoute, previewRoute])
export const router = createRouter({ routeTree })

function Explorer() {
  const search = indexRoute.useSearch()
  const navigate = useNavigate({ from: indexRoute.fullPath })
  const [blueprint, setBlueprint] = useState(search.blueprint ?? '')
  const [version, setVersion] = useState(search.version?.toString() ?? '')
  const [query, setQuery] = useState(search.query ?? '')
  const results = useQuery({
    queryKey: ['entities', search.blueprint, search.version, search.query],
    queryFn: () => searchEntities(search.blueprint!, search.version, search.query ?? ''),
    enabled: Boolean(search.blueprint),
  })

  function submit(event: FormEvent) {
    event.preventDefault()
    const next = parseExplorerSearch({ blueprint, version, query })
    void navigate({ to: '/', search: next })
  }

  const columnHelper = legacyCreateColumnHelper<EntityItem>()
  const columns: LegacyColumnDef<EntityItem, any>[] = [
    columnHelper.accessor('code', { header: 'Code', cell: (info) => <Link to="/entities/$entityId" params={{ entityId: info.row.original.id }}>{info.getValue()}</Link> }),
    ...(results.data?.blueprint.attributes ?? []).map((attribute) => columnHelper.display({
      id: attribute.code,
      header: attribute.code,
      cell: (info) => displayValue(info.row.original.preview[attribute.code]),
    })),
  ]
  const table = useLegacyTable({ data: results.data?.items ?? [], columns, getCoreRowModel: getCoreRowModel() })

  return <main className="shell">
    <header>
      <p className="eyebrow">Catalog</p>
      <h1>Entity explorer</h1>
      <p className="intro">Search a blueprint and inspect its current entity projections.</p>
    </header>
    <form className="search-form" onSubmit={submit}>
      <label>Blueprint<input required value={blueprint} onChange={(event) => setBlueprint(event.target.value)} placeholder="product" /></label>
      <label>Version <input inputMode="numeric" min="1" value={version} onChange={(event) => setVersion(event.target.value)} placeholder="Current" /></label>
      <label className="query-field">Query<input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Search terms" /></label>
      <button type="submit">Search</button>
    </form>
    {!search.blueprint && <p className="empty">Enter a blueprint code to start exploring.</p>}
    {results.isPending && <p className="empty">Loading entities...</p>}
    {results.isError && <p className="error">{results.error.message}</p>}
    {results.data && <section className="results" aria-live="polite">
      <div className="result-meta"><strong>{results.data.blueprint.blueprint.code}</strong> v{results.data.blueprint.blueprint.version} · {results.data.items.length} result{results.data.items.length === 1 ? '' : 's'}</div>
      <div className="table-wrap"><table>
        <thead>{table.getHeaderGroups().map((group) => <tr key={group.id}>{group.headers.map((header) => <th key={header.id}>{header.isPlaceholder ? null : flexRender(header.column.columnDef.header, header.getContext())}</th>)}</tr>)}</thead>
        <tbody>{table.getRowModel().rows.map((row) => <tr key={row.id}>{row.getVisibleCells().map((cell) => <td key={cell.id}>{flexRender(cell.column.columnDef.cell, cell.getContext())}</td>)}</tr>)}</tbody>
      </table></div>
      {results.data.items.length === 0 && <p className="empty">No entities matched this search.</p>}
      {results.data.next_cursor && <p className="muted">More results are available. Pagination will be added with the search cursor.</p>}
    </section>}
  </main>
}

function EntityPreview() {
  const { entityId } = previewRoute.useParams()
  const preview = useQuery({ queryKey: ['entity-preview', entityId], queryFn: () => getEntityPreview(entityId) })
  return <main className="shell detail">
    <Link to="/" className="back">← Explorer</Link>
    <p className="eyebrow">Entity preview</p>
    <h1>{preview.data?.code ?? entityId}</h1>
    {preview.isPending && <p className="empty">Loading preview...</p>}
    {preview.isError && <p className="error">{preview.error.message}</p>}
    {preview.data && <pre>{JSON.stringify(preview.data, null, 2)}</pre>}
  </main>
}
