import { useQuery } from '@tanstack/react-query'
import { Link, createFileRoute } from '@tanstack/react-router'
import { Alert, Box, Button, Container, Paper, Typography } from '@mui/material'
import { getEntityPreview } from '../../api'

export const Route = createFileRoute('/entities/$entityId')({
  component: EntityPreview,
})

function EntityPreview() {
  const { entityId } = Route.useParams()
  const preview = useQuery({
    queryKey: ['entity-preview', entityId],
    queryFn: () => getEntityPreview(entityId),
  })
  return (
    <Container component="main" maxWidth="lg" sx={{ py: { xs: 4, md: 7 } }}>
      <Button component={Link} to="/" sx={{ mb: 4 }}>
        Back to explorer
      </Button>
      <Typography
        color="primary"
        sx={{
          fontWeight: 700,
          letterSpacing: '.12em',
          textTransform: 'uppercase',
        }}
        variant="overline"
      >
        Entity preview
      </Typography>
      <Typography component="h1" variant="h3">
        {entityId}
      </Typography>
      <Box sx={{ mt: 1 }}>
        <Link params={{ entityId }} to="/entities/$entityId/edit">
          Edit entity
        </Link>
      </Box>
      {preview.isPending && (
        <Typography sx={{ py: 3 }}>Loading preview...</Typography>
      )}
      {preview.isError && (
        <Alert severity="error" sx={{ mt: 3 }}>
          {preview.error.message}
        </Alert>
      )}
      {preview.data && (
        <Paper component="pre" sx={{ mt: 3, overflow: 'auto', p: 3 }}>
          {JSON.stringify(preview.data, null, 2)}
        </Paper>
      )}
    </Container>
  )
}
