import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import {
  Alert,
  Button,
  Container,
  List,
  ListItem,
  ListItemText,
  Paper,
  Typography,
} from '@mui/material';
import { listContexts } from '../entities/api';
import { entityQueryKeys } from '../entities/query-keys';

export const ContextsPage = () => {
  const contexts = useQuery({
    queryKey: entityQueryKeys.contexts(),
    queryFn: listContexts,
  });
  return (
    <Container component="main" maxWidth="md" sx={{ py: { xs: 4, md: 7 } }}>
      <Button component={Link} to="/" sx={{ mb: 4 }}>
        Back to explorer
      </Button>
      <Typography component="h1" variant="h3">
        Contexts
      </Typography>
      <Button
        component={Link}
        sx={{ mt: 3 }}
        to="/contexts/new"
        variant="contained"
      >
        Create context
      </Button>
      {contexts.isPending && (
        <Typography sx={{ mt: 3 }}>Loading contexts...</Typography>
      )}
      {contexts.isError && (
        <Alert severity="error" sx={{ mt: 3 }}>
          {contexts.error.message}
        </Alert>
      )}
      {contexts.data && (
        <Paper sx={{ mt: 3 }}>
          <List disablePadding>
            {contexts.data.map((context) => (
              <ListItem divider key={context.id}>
                <ListItemText
                  primary={context.code}
                  secondary={`${context.parent_id ? `Parent: ${contexts.data.find((parent) => parent.id === context.parent_id)?.code ?? 'unknown'} · ` : 'Root · '}${JSON.stringify(context.data)}`}
                />
              </ListItem>
            ))}
            {!contexts.data.length && (
              <ListItem>
                <ListItemText primary="No contexts yet." />
              </ListItem>
            )}
          </List>
        </Paper>
      )}
    </Container>
  );
};
