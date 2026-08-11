import { Link } from '@tanstack/react-router'
import { Button, Container, Typography } from '@mui/material'
import type { ReactNode } from 'react'

export function EntityPage({
  children,
  title,
}: {
  children: ReactNode
  title: string
}) {
  return (
    <Container component="main" maxWidth="md" sx={{ py: { xs: 4, md: 7 } }}>
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
        Catalog
      </Typography>
      <Typography component="h1" variant="h3">
        {title}
      </Typography>
      {children}
    </Container>
  )
}
