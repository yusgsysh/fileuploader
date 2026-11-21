import {
  createContext,
  useContext,
  useEffect,
  type ReactNode,
} from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { api, queryKeys } from "@/lib/api";
import type { AuthStatus } from "@/types";

interface AuthContextValue {
  status?: AuthStatus;
  isLoading: boolean;
  refresh: () => Promise<void>;
  logout: () => Promise<void>;
}

const AuthContext = createContext<AuthContextValue | null>(null);

export function AuthProvider({ children }: { children: ReactNode }) {
  const queryClient = useQueryClient();
  const query = useQuery({
    queryKey: queryKeys.auth,
    queryFn: api.authStatus,
    retry: false,
    staleTime: 30_000,
  });

  useEffect(() => {
    const handler = () => {
      void query.refetch();
    };
    window.addEventListener("fu:unauthorized", handler);
    return () => window.removeEventListener("fu:unauthorized", handler);
  }, [query.refetch]);

  const refresh = async () => {
    await query.refetch();
  };

  const logout = async () => {
    await api.logout();
    queryClient.clear();
    await query.refetch();
  };

  return (
    <AuthContext.Provider
      value={{
        status: query.data,
        isLoading: query.isLoading,
        refresh,
        logout,
      }}
    >
      {children}
    </AuthContext.Provider>
  );
}

export function useAuth() {
  const context = useContext(AuthContext);
  if (!context) {
    throw new Error("useAuth must be used inside AuthProvider");
  }
  return context;
}
