import { betterAuth } from "better-auth";
import { prismaAdapter } from "better-auth/adapters/prisma";
import { prisma } from "./db";

const allowedEmails = (process.env.ALLOWED_EMAILS || "")
  .split(",")
  .map((email) => email.trim().toLowerCase())
  .filter(Boolean);

export const auth = betterAuth({
  secret: process.env.BETTER_AUTH_SECRET,
  baseURL: process.env.BETTER_AUTH_URL,
  database: prismaAdapter(prisma, {
    provider: "sqlite",
  }),
  socialProviders: {
    google: {
      clientId: process.env.GOOGLE_CLIENT_ID!,
      clientSecret: process.env.GOOGLE_CLIENT_SECRET!,
    },
  },
  trustedOrigins: [process.env.BETTER_AUTH_URL || "http://localhost:3000"],
  user: {
    additionalFields: {},
    changeEmail: { enabled: false },
    deleteUser: { enabled: false },
  },
  account: {
    accountLinking: { enabled: true },
  },
  callbacks: {
    async onUserCreate({ user }: { user: { id: string; email: string } }) {
      if (allowedEmails.length > 0 && !allowedEmails.includes(user.email.toLowerCase())) {
        await prisma.user.delete({ where: { id: user.id } });
        throw new Error("User not allowed");
      }
      return user;
    },
  },
});
